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

/// Boolean trio: a block and a fat cylinder sliding through it while
/// the operation cycles union -> intersection -> difference (12 frames
/// each). Every frame is a real boolean.
fn trio(dir: &Path) {
    let frames = 36usize;
    for i in 0..frames {
        let seg = i / 12; // 0 union, 1 intersection, 2 difference
        let t = (i % 12) as f64 / 12.0;
        // The tool slides diagonally across the block within each
        // segment, staying clear of the side walls (the interior
        // bore family).
        let cx = 1.4 + 1.2 * t;
        let mut block = Body::new();
        block.block(Vec3::ZERO, 4.0, 4.0, 2.0).unwrap();
        let f = Frame3::from_z(Vec3::new(cx, cx, -0.75), Vec3::new(0.0, 0.0, 1.0)).unwrap();
        let mut tool = Body::new();
        tool.cylinder(f, 1.3, 3.5).unwrap();
        let (op, name) = match seg {
            0 => (BoolOp::Union, "union"),
            1 => (BoolOp::Intersection, "intersection"),
            _ => (BoolOp::Difference, "difference"),
        };
        let body = boolean(&block, &tool, op, 1e-7)
            .expect("trio frame declined")
            .body;
        let mesh = body.worker_mesh();
        write_frame(dir, i, &mesh, &format!("\"label\":\"{name}\""));
    }
    println!("trio: {frames} frames");
}

/// THE thesis gif: the clearance pin. A pin visibly smaller than the
/// hole grows toward the real-world export gap (1e-5), the strict
/// kernel honestly shows two parts, then the TOLERANT boolean snaps the
/// pin onto the hole's exact carrier and the parts fuse seamlessly
/// (salvaged tier 2, surfaced in the badge).
fn pin(dir: &Path) {
    use keel_topo::boolean::boolean_tolerant;
    let frames = 36usize;
    let mut plate = Body::new();
    plate.block(Vec3::ZERO, 4.0, 4.0, 1.0).unwrap();
    let df = Frame3::from_z(Vec3::new(2.0, 2.0, -0.5), Vec3::new(0.0, 0.0, 1.0)).unwrap();
    let mut tool = Body::new();
    tool.cylinder(df, 1.0, 2.0).unwrap();
    let holed = boolean(&plate, &tool, BoolOp::Difference, 1e-7)
        .expect("holed plate")
        .body;
    for i in 0..frames {
        let approach = 26usize;
        if i < approach {
            // The pin grows toward the fit: honestly TWO bodies (the
            // strict kernel will not pretend a gap is a mate). Frames
            // are the two worker meshes concatenated.
            let t = i as f64 / (approach - 1) as f64;
            let r = 0.55 + (1.0 - 1e-5 - 0.55) * t * t;
            let pf = Frame3::from_z(Vec3::new(2.0, 2.0, 0.0), Vec3::new(0.0, 0.0, 1.0)).unwrap();
            let mut pin = Body::new();
            pin.cylinder(pf, r, 1.0).unwrap();
            let mut mesh = holed.worker_mesh();
            let pm = pin.worker_mesh();
            let base = (mesh.positions.len() / 3) as u32;
            mesh.positions.extend_from_slice(&pm.positions);
            mesh.normals.extend_from_slice(&pm.normals);
            mesh.indices.extend(pm.indices.iter().map(|&k| k + base));
            mesh.lines.extend_from_slice(&pm.lines);
            write_frame(dir, i, &mesh, &format!("\"gap\":{:.6}", 1.0 - r));
        } else {
            // The tolerant snap: ONE exact body, gap salvaged.
            let pf = Frame3::from_z(Vec3::new(2.0, 2.0, 0.0), Vec3::new(0.0, 0.0, 1.0)).unwrap();
            let mut pin = Body::new();
            pin.cylinder(pf, 1.0 - 1e-5, 1.0).unwrap();
            let (r, conf) = boolean_tolerant(&holed, &pin, BoolOp::Union, 1e-7, 1e-4)
                .expect("tolerant pin declined");
            let mesh = r.body.worker_mesh();
            write_frame(
                dir,
                i,
                &mesh,
                &format!(
                    "\"salvaged\":{},\"tier\":{},\"achieved\":{:e}",
                    conf.salvaged, conf.tier, conf.achieved_tolerance
                ),
            );
        }
    }
    println!("pin: {frames} frames");
}

/// The fillet overflow ladder: the radius grows past the supporting
/// face's width. The ordinary fillet handles the small radii; past the
/// trigger it DECLINES (never guesses) and the cliff handler engages,
/// rolling over the far edge (the configuration where OCCT's fillet
/// fails outright). The sweep skips the trigger's immediate
/// neighborhood (the honest both-decline band).
fn fillet(dir: &Path) {
    use keel_topo::entity::EdgeKey;
    let frames = 36usize;
    for i in 0..frames {
        let t = i as f64 / (frames - 1) as f64;
        let r = if t < 0.55 {
            0.30 + (0.95 - 0.30) * (t / 0.55)
        } else {
            1.10 + (1.55 - 1.10) * ((t - 0.55) / 0.45)
        };
        let mut b = Body::new();
        b.block(Vec3::ZERO, 4.0, 1.0, 2.0).unwrap();
        // The wall-top edge of the 4x1x2 block: midpoint (2, 0, 2).
        let e: EdgeKey = b
            .entity_ids()
            .filter_map(|id| match b.lookup(id) {
                Some(keel_topo::entity::AnyKey::Edge(k)) => Some(k),
                _ => None,
            })
            .find(|&k| {
                let Some(edge) = b.edge(k) else { return false };
                let (Some(p0), Some(p1)) = (
                    b.vertex(edge.bounds.0).map(|v| v.point),
                    b.vertex(edge.bounds.1).map(|v| v.point),
                ) else {
                    return false;
                };
                let m = (p0 + p1) * 0.5;
                (m - Vec3::new(2.0, 0.0, 2.0)).norm() < 1e-9
            })
            .expect("wall-top edge");
        let (body, label) = match b.fillet_edge(e, r) {
            Ok(x) => (x, "fillet".to_string()),
            Err(_) => (
                b.fillet_edge_cliff(e, r).expect("ladder frame declined"),
                "overflow: cliff handler".to_string(),
            ),
        };
        let mesh = body.worker_mesh();
        write_frame(dir, i, &mesh, &format!("\"label\":\"{label} r={r:.2}\""));
    }
    println!("fillet: {frames} frames");
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
        "trio" => trio(dir),
        "pin" => pin(dir),
        "fillet" => fillet(dir),
        other => {
            eprintln!("unknown op {other}; available: drill, trio, pin, fillet");
            std::process::exit(1);
        }
    }
}
