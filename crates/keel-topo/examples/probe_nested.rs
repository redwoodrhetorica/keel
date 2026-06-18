//! Diagnose the nested-op collapse: are COMPOUND-operand booleans weak, or did
//! the random session just collide features? Here every feature is DELIBERATELY
//! spaced (a grid), non-overlapping, the way a real user places them. Each op is
//! block-with-prior-features (op) fresh-primitive. If these land, the random
//! test's low rate is feature-collision; if they decline, compound operands are
//! the real gap.
//! Run: cargo run --release -p keel-topo --example probe_nested
#![allow(clippy::unwrap_used)]
use keel_geom::surface::Frame3;
use keel_math::vec::Vec3;
use keel_topo::Body;
use keel_topo::boolean::{BoolOp, boolean};

fn cyl(pos: Vec3, r: f64, h: f64) -> Body {
    let mut b = Body::new();
    b.cylinder(Frame3::from_z(pos, Vec3::new(0., 0., 1.)).unwrap(), r, h)
        .unwrap();
    b
}
fn blk(o: Vec3, dx: f64, dy: f64, dz: f64) -> Body {
    let mut b = Body::new();
    b.block(o, dx, dy, dz).unwrap();
    b
}

fn run(label: &str, mut body: Body, tools: Vec<(Body, BoolOp)>) {
    let v0 = body.mass_properties().map(|m| m.volume).unwrap_or(f64::NAN);
    let n = tools.len();
    let mut landed = 0;
    let mut first_fail = None;
    for (i, (tool, op)) in tools.into_iter().enumerate() {
        match boolean(&body, &tool, op, 1e-7) {
            Err(e) => {
                if first_fail.is_none() {
                    first_fail = Some(format!("op {i}: DECLINED {e:?}"));
                }
            }
            Ok(r) if !r.faults.is_empty() => {
                if first_fail.is_none() {
                    first_fail = Some(format!("op {i}: faults {:?}", r.faults));
                }
            }
            Ok(r) => {
                if r.body.validate().is_ok() {
                    body = r.body;
                    landed += 1;
                } else if first_fail.is_none() {
                    first_fail = Some(format!("op {i}: INVALID shell"));
                }
            }
        }
    }
    let vf = body.mass_properties().map(|m| m.volume).unwrap_or(f64::NAN);
    println!(
        "{label}: landed {landed}/{n}  vol {v0:.2} -> {vf:.2}  {}",
        first_fail.unwrap_or_default()
    );
}

fn main() {
    // A: 16 well-spaced through-holes in a plate (4x4 grid, r0.8, 4 apart).
    let plate = blk(Vec3::ZERO, 20., 20., 3.);
    let mut holes = Vec::new();
    for ix in 0..4 {
        for iy in 0..4 {
            let (x, y) = (2.5 + ix as f64 * 5.0, 2.5 + iy as f64 * 5.0);
            holes.push((cyl(Vec3::new(x, y, -0.5), 0.8, 4.0), BoolOp::Difference));
        }
    }
    run("A spaced through-holes", plate.clone(), holes);

    // B: 9 spaced pockets.
    let mut pockets = Vec::new();
    for ix in 0..3 {
        for iy in 0..3 {
            let (x, y) = (2.0 + ix as f64 * 6.0, 2.0 + iy as f64 * 6.0);
            pockets.push((blk(Vec3::new(x, y, 1.5), 2.0, 2.0, 2.0), BoolOp::Difference));
        }
    }
    run("B spaced pockets", plate.clone(), pockets);

    // C: 9 spaced cylindrical bosses (union).
    let mut bosses = Vec::new();
    for ix in 0..3 {
        for iy in 0..3 {
            let (x, y) = (2.5 + ix as f64 * 6.0, 2.5 + iy as f64 * 6.0);
            bosses.push((cyl(Vec3::new(x, y, 2.9), 1.0, 2.0), BoolOp::Union));
        }
    }
    run("C spaced bosses", plate.clone(), bosses);

    // D: alternating holes + bosses (mixed), spaced.
    let mut mixed = Vec::new();
    for ix in 0..4 {
        for iy in 0..4 {
            let (x, y) = (2.5 + ix as f64 * 5.0, 2.5 + iy as f64 * 5.0);
            if (ix + iy) % 2 == 0 {
                mixed.push((cyl(Vec3::new(x, y, -0.5), 0.8, 4.0), BoolOp::Difference));
            } else {
                mixed.push((cyl(Vec3::new(x, y, 2.9), 0.8, 1.5), BoolOp::Union));
            }
        }
    }
    run("D spaced mixed", plate.clone(), mixed);
}
