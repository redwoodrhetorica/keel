//! Frame dumper for the README op GIFs (task 34): runs a parameter
//! sweep for one op, tessellates each frame through the kernel's own
//! worker_mesh (the honest-output rule: frames come from the kernel,
//! not a re-modelled approximation), and writes one compact JSON per
//! frame for the Python renderer (docs/media/render_gif.py).
//!
//! Usage: cargo run --release -p keel-topo --example gif_frames -- <op> <outdir>
//! Ops: drill
#![allow(clippy::unwrap_used, clippy::expect_used)]

use keel_geom::surface::Frame3;
use keel_math::vec::Vec3;
use keel_topo::body::Body;
use keel_topo::boolean::{BoolOp, boolean};
use keel_topo::render::WorkerMesh;
use std::fmt::Write as _;
use std::path::Path;

fn json_f32s(v: &[f32]) -> String {
    let mut s = String::with_capacity(v.len() * 8);
    s.push('[');
    for (i, x) in v.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        let _ = write!(s, "{x}");
    }
    s.push(']');
    s
}

fn json_u32s(v: &[u32]) -> String {
    let mut s = String::with_capacity(v.len() * 6);
    s.push('[');
    for (i, x) in v.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        let _ = write!(s, "{x}");
    }
    s.push(']');
    s
}

fn write_frame(dir: &Path, idx: usize, mesh: &WorkerMesh, meta: &str) {
    let mut s = String::new();
    s.push('{');
    let _ = write!(s, "\"positions\":{},", json_f32s(&mesh.positions));
    let _ = write!(s, "\"normals\":{},", json_f32s(&mesh.normals));
    let _ = write!(s, "\"indices\":{},", json_u32s(&mesh.indices));
    let _ = write!(s, "\"lines\":{},", json_f32s(&mesh.lines));
    let _ = write!(s, "\"meta\":{{{meta}}}");
    s.push('}');
    std::fs::write(dir.join(format!("frame_{idx:03}.json")), s).unwrap();
}

/// Drill difference: a plate drilled by a cylinder whose radius grows,
/// then shrinks back (a seamless loop). Every frame is a REAL boolean.
fn drill(dir: &Path) {
    let frames = 36usize;
    for i in 0..frames {
        // Triangle wave 0..1..0 across the loop.
        let t = i as f64 / frames as f64;
        let w = 1.0 - (2.0 * t - 1.0).abs();
        let r = 0.25 + 1.25 * w;
        let mut plate = Body::new();
        plate.block(Vec3::ZERO, 4.0, 4.0, 1.0).unwrap();
        let f = Frame3::from_z(Vec3::new(2.0, 2.0, -0.5), Vec3::new(0.0, 0.0, 1.0)).unwrap();
        let mut tool = Body::new();
        tool.cylinder(f, r, 2.0).unwrap();
        let body = boolean(&plate, &tool, BoolOp::Difference, 1e-7)
            .expect("drill frame declined")
            .body;
        let mesh = body.worker_mesh();
        write_frame(dir, i, &mesh, &format!("\"radius\":{r}"));
    }
    println!("drill: {frames} frames");
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let op = args.get(1).map(String::as_str).unwrap_or("drill");
    let out = args
        .get(2)
        .cloned()
        .unwrap_or_else(|| format!("docs/media/frames/{op}"));
    let dir = Path::new(&out);
    std::fs::create_dir_all(dir).unwrap();
    match op {
        "drill" => drill(dir),
        other => {
            eprintln!("unknown op {other}; available: drill");
            std::process::exit(1);
        }
    }
}
