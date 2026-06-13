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

/// Cellular (non-regularized) union: two blocks overlap and the shared
/// interface survives as a DOUBLE-SIDED interior partition wall (item
/// 29 rung 1): the result is a cellular solid, not a soup. The blocks
/// slide together through the loop; the renderer draws the wall's
/// edges through the glass body.
fn cellular(dir: &Path) {
    use keel_topo::boolean::{BooleanOptions, boolean_with};
    let frames = 36usize;
    for i in 0..frames {
        let t = i as f64 / frames as f64;
        let w = 1.0 - (2.0 * t - 1.0).abs();
        // B slides from barely-touching to half-overlapped and back.
        let x = 2.0 - 1.0 * w;
        let mut a = Body::new();
        a.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        let mut b = Body::new();
        b.block(Vec3::new(x, 0.0, 0.0), 2.0, 2.0, 2.0).unwrap();
        let body = boolean_with(
            &a,
            &b,
            BoolOp::Union,
            1e-7,
            BooleanOptions { regularize: false },
        )
        .expect("cellular frame declined")
        .body;
        let mesh = body.worker_mesh();
        write_frame(dir, i, &mesh, "\"label\":\"non-regularized: wall kept\"");
    }
    println!("cellular: {frames} frames");
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

/// The octant corner blend: a sphere patch closing the three fillets
/// meeting at a cube corner, radius breathing through a loop. (The
/// ghost-circle wireframe issue that parked this demo was fixed by
/// recording short-arc sweeps at construction, task 29 era.)
fn corner(dir: &Path) {
    let frames = 36usize;
    for i in 0..frames {
        let t = i as f64 / frames as f64;
        let w = 1.0 - (2.0 * t - 1.0).abs();
        let r = 0.25 + 0.65 * w;
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        let corner = b
            .entity_ids()
            .filter_map(|id| match b.lookup(id) {
                Some(keel_topo::entity::AnyKey::Vertex(k)) => Some(k),
                _ => None,
            })
            .find(|&k| {
                b.vertex(k)
                    .map(|v| (v.point - Vec3::new(2.0, 2.0, 2.0)).norm() < 1e-9)
                    .unwrap_or(false)
            })
            .expect("cube corner");
        let body = b
            .fillet_corner_octant(corner, r)
            .expect("corner frame declined");
        let mesh = body.worker_mesh();
        write_frame(
            dir,
            i,
            &mesh,
            &format!("\"label\":\"corner blend r={r:.2}\""),
        );
    }
    println!("corner: {frames} frames");
}

/// The honest-DECLINE oddball, the sibling of the steinmetz demo: a
/// THIN cylinder slides across a FAT one until their axes cross. EQUAL
/// radii give the exact bicylinder (the steinmetz demo); UNEQUAL radii
/// produce asymmetric intersection ellipses the crossing-pair imprint
/// does not yet assemble, so instead of emitting a plausible-but-wrong
/// body the kernel DECLINES, on camera. The frames show the two honest
/// input cylinders; the finale labels the refusal. (The original
/// sphere-socket version became a success story when task 36 landed;
/// this is a configuration that still genuinely declines.)
fn decline(dir: &Path) {
    let frames = 36usize;
    let h = 4.0;
    let mut fat = Body::new();
    fat.cylinder(
        Frame3::from_z(Vec3::new(0.0, 0.0, -h * 0.5), Vec3::new(0.0, 0.0, 1.0)).unwrap(),
        1.0,
        h,
    )
    .unwrap();
    for i in 0..frames {
        // The thin cylinder slides in along x until its axis crosses
        // the fat one's at the origin, then attempts the intersection.
        let approach = 26usize;
        let cx = if i < approach {
            let t = i as f64 / (approach - 1) as f64;
            -3.5 + 3.5 * t * t
        } else {
            0.0
        };
        let mut thin = Body::new();
        thin.cylinder(
            Frame3::from_z(Vec3::new(cx - h * 0.5, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0)).unwrap(),
            0.55,
            h,
        )
        .unwrap();
        let attempted = i >= approach;
        let declined = attempted && boolean(&fat, &thin, BoolOp::Intersection, 1e-7).is_err();
        let mut mesh = fat.worker_mesh();
        let tm = thin.worker_mesh();
        let base = (mesh.positions.len() / 3) as u32;
        mesh.positions.extend_from_slice(&tm.positions);
        mesh.normals.extend_from_slice(&tm.normals);
        mesh.indices.extend(tm.indices.iter().map(|&k| k + base));
        mesh.lines.extend_from_slice(&tm.lines);
        let meta = if declined {
            "\"declined\":true".to_string()
        } else if attempted {
            // If the kernel ever assembles this, the gif must not keep
            // claiming a decline (honest-output rule).
            "\"label\":\"assembled\"".to_string()
        } else {
            String::new()
        };
        write_frame(dir, i, &mesh, &meta);
    }
    println!("decline: {frames} frames");
}

/// Extrude + revolve: a tapered pentagon tower morphs (top scale
/// breathing), then a revolved vase whose bulge breathes. Both halves
/// are real constructions, not booleans.
fn construct(dir: &Path) {
    let frames = 36usize;
    for i in 0..frames {
        let half = 18usize;
        let (body, label) = if i < half {
            let t = i as f64 / half as f64;
            let w = 1.0 - (2.0 * t - 1.0).abs();
            let s = 1.0 - 0.6 * w;
            let base: Vec<Vec3> = (0..5)
                .map(|k| {
                    let a = core::f64::consts::TAU * k as f64 / 5.0 + core::f64::consts::FRAC_PI_2;
                    Vec3::new(a.cos(), a.sin(), 0.0)
                })
                .collect();
            let mut b = Body::new();
            b.extrude_tapered(&base, Vec3::new(0.0, 0.0, 2.0), s)
                .expect("extrude frame");
            (b, format!("extrude taper {s:.2}"))
        } else {
            let t = (i - half) as f64 / half as f64;
            let w = 1.0 - (2.0 * t - 1.0).abs();
            let bulge = 0.75 + 0.45 * w;
            let f = Frame3::from_z(Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0)).unwrap();
            let mut b = Body::new();
            b.revolve(
                f,
                &[
                    (0.0, 0.0),
                    (0.55, 0.0),
                    (bulge, 0.7),
                    (0.45, 1.5),
                    (0.5, 2.0),
                    (0.0, 2.0),
                ],
            )
            .expect("revolve frame");
            (b, "revolve".to_string())
        };
        let mesh = body.worker_mesh();
        write_frame(dir, i, &mesh, &format!("\"label\":\"{label}\""));
    }
    println!("construct: {frames} frames");
}

/// Shell/hollow cutaway: a block hollows to a breathing wall thickness
/// and a guillotine split reveals the cavity (both ops real: hollow,
/// then split_by_plane keeping the back piece).
fn shell(dir: &Path) {
    let frames = 36usize;
    for i in 0..frames {
        let t = i as f64 / frames as f64;
        let w = 1.0 - (2.0 * t - 1.0).abs();
        let th = 0.12 + 0.38 * w;
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.4, 2.4, 1.8).unwrap();
        let hollowed = b.hollow(th).expect("hollow frame");
        let (_back, front) = hollowed
            .split_by_plane(Vec3::new(1.2, 1.2, 0.9), Vec3::new(-1.0, 1.0, 0.0))
            .expect("cutaway split");
        let mesh = front.worker_mesh();
        write_frame(dir, i, &mesh, &format!("\"label\":\"hollow t={th:.2}\""));
    }
    println!("shell: {frames} frames");
}

/// Offset breathing: an L-plate offsets outward and back (planar
/// scope), every wall moving along its own normal: the concave corner
/// shows the offset re-solving the intersections, not scaling.
fn offset(dir: &Path) {
    let frames = 36usize;
    for i in 0..frames {
        let t = i as f64 / frames as f64;
        let d = 0.30 * (core::f64::consts::TAU * t).sin();
        let mut plate = Body::new();
        plate.block(Vec3::ZERO, 4.0, 4.0, 1.2).unwrap();
        let mut notch = Body::new();
        notch
            .block(Vec3::new(1.8, 1.8, -0.5), 3.0, 3.0, 2.2)
            .unwrap();
        let mut body = boolean(&plate, &notch, BoolOp::Difference, 1e-7)
            .expect("offset base declined")
            .body;
        body.offset_body(d).expect("offset frame");
        let mesh = body.worker_mesh();
        write_frame(dir, i, &mesh, &format!("\"label\":\"offset {d:+.2}\""));
    }
    println!("offset: {frames} frames");
}

/// The GWN classification cloud: a sheet of probe points sweeps up
/// through a drilled plate, each point colored by the body's
/// generalized winding number (green inside, grey outside): the bore
/// reads correctly hollow, on-boundary noise nowhere in sight: the
/// classifier the boolean pipeline trusts, made visible.
fn gwn(dir: &Path) {
    let frames = 36usize;
    let mut plate = Body::new();
    plate.block(Vec3::ZERO, 4.0, 4.0, 1.2).unwrap();
    let f = Frame3::from_z(Vec3::new(2.0, 2.0, -0.5), Vec3::new(0.0, 0.0, 1.0)).unwrap();
    let mut tool = Body::new();
    tool.cylinder(f, 1.0, 2.5).unwrap();
    let body = boolean(&plate, &tool, BoolOp::Difference, 1e-7)
        .expect("gwn base declined")
        .body;
    let mesh = body.worker_mesh();
    for i in 0..frames {
        let t = i as f64 / frames as f64;
        let w01 = 1.0 - (2.0 * t - 1.0).abs();
        let z = -0.35 + (1.55 + 0.35) * w01;
        let n = 14usize;
        let mut pts = String::from("[");
        let mut ws = String::from("[");
        for iy in 0..n {
            for ix in 0..n {
                let p = Vec3::new(
                    0.15 + 3.7 * ix as f64 / (n - 1) as f64,
                    0.15 + 3.7 * iy as f64 / (n - 1) as f64,
                    z,
                );
                let w = body.generalized_winding_number(p);
                if pts.len() > 1 {
                    pts.push(',');
                    ws.push(',');
                }
                let _ = write!(pts, "{},{},{}", p.x, p.y, p.z);
                let _ = write!(ws, "{w:.3}");
            }
        }
        pts.push(']');
        ws.push(']');
        write_frame(
            dir,
            i,
            &mesh,
            &format!("\"label\":\"winding number\",\"points\":{pts},\"winding\":{ws}"),
        );
    }
    println!("gwn: {frames} frames");
}

/// Wire imprint + face push: a rectangle imprints onto the block's top
/// face (real topology, not a sketch overlay), then the imprinted face
/// pushes up and down through the loop (offset_face): the elemental
/// feature-modeling pair.
fn imprint_push(dir: &Path) {
    let frames = 36usize;
    for i in 0..frames {
        let t = i as f64 / frames as f64;
        let d = 0.9 * (core::f64::consts::TAU * t).sin();
        let mut b = Body::new();
        b.block(Vec3::ZERO, 4.0, 4.0, 1.5).unwrap();
        let top = b
            .pick_face(Vec3::new(2.0, 2.0, 1.5), 1e-7)
            .expect("top face");
        let ring = [
            Vec3::new(1.0, 1.0, 1.5),
            Vec3::new(3.0, 1.0, 1.5),
            Vec3::new(3.0, 3.0, 1.5),
            Vec3::new(1.0, 3.0, 1.5),
        ];
        b.imprint_closed_polyline(top, &ring).expect("imprint");
        let pocket = b
            .pick_face(Vec3::new(2.0, 2.0, 1.5), 1e-7)
            .expect("imprinted face");
        if d.abs() > 1e-9 {
            b.push_face(pocket, d).expect("push frame");
        }
        let mesh = b.worker_mesh();
        write_frame(
            dir,
            i,
            &mesh,
            &format!("\"label\":\"imprint + push {d:+.2}\""),
        );
    }
    println!("imprint_push: {frames} frames");
}

/// SGC merge: the non-regularized union keeps the interior wall as the
/// blocks slide together (the cellular model); merge_cells then
/// dissolves the wall and the cells fuse into one region, on camera.
fn merge(dir: &Path) {
    use keel_topo::boolean::{BooleanOptions, boolean_with};
    let frames = 36usize;
    let touch = 18usize;
    let dissolve = 27usize;
    for i in 0..frames {
        // Approach until the faces COINCIDE (the item-29 abutting
        // configuration), hold the cellular wall, then dissolve.
        let t = (i.min(touch) as f64) / touch as f64;
        let x = 1.0 + 1.0 * (1.0 - t);
        let mut a = Body::new();
        a.block(Vec3::ZERO, 1.0, 1.0, 1.0).unwrap();
        let mut b = Body::new();
        b.block(Vec3::new(x, 0.0, 0.0), 1.0, 1.0, 1.0).unwrap();
        let mut body = boolean_with(
            &a,
            &b,
            BoolOp::Union,
            1e-7,
            BooleanOptions { regularize: false },
        )
        .expect("merge frame declined")
        .body;
        let label = if i >= dissolve {
            let n = body.merge_cells().expect("merge_cells");
            format!("merge_cells: {n} wall dissolved")
        } else if i >= touch {
            "cellular: wall kept".to_string()
        } else {
            String::from("approach")
        };
        let mesh = body.worker_mesh();
        write_frame(dir, i, &mesh, &format!("\"label\":\"{label}\""));
    }
    println!("merge: {frames} frames");
}

/// The sphere socket SUCCESS story (task 36): the ball sinks, the REAL
/// carve engages (every carved frame is a real difference, exact mass),
/// then the mated ball re-unions into the socket (coincident on-on).
fn socket(dir: &Path) {
    let frames = 36usize;
    let mut block = Body::new();
    block.block(Vec3::ZERO, 4.0, 4.0, 2.0).unwrap();
    for i in 0..frames {
        let approach = 14usize;
        let carved = 25usize;
        let zc = if i < approach {
            let t = i as f64 / (approach - 1) as f64;
            3.2 - (3.2 - 1.5) * t * t
        } else {
            1.5
        };
        let mut ball = Body::new();
        ball.sphere(
            Frame3::from_z(Vec3::new(2.0, 2.0, zc), Vec3::new(0.0, 0.0, 1.0)).unwrap(),
            1.0,
        )
        .unwrap();
        let (mesh, label) = if i < approach {
            let mut mesh = block.worker_mesh();
            let bm = ball.worker_mesh();
            let base = (mesh.positions.len() / 3) as u32;
            mesh.positions.extend_from_slice(&bm.positions);
            mesh.normals.extend_from_slice(&bm.normals);
            mesh.indices.extend(bm.indices.iter().map(|&k| k + base));
            mesh.lines.extend_from_slice(&bm.lines);
            (mesh, "approach".to_string())
        } else if i < carved {
            let sunk = boolean(&block, &ball, BoolOp::Difference, 1e-7)
                .expect("socket carve declined")
                .body;
            let v = sunk.mass_properties().expect("socket mass").volume;
            (sunk.worker_mesh(), format!("carved, mass {v:.6}"))
        } else {
            let sunk = boolean(&block, &ball, BoolOp::Difference, 1e-7)
                .expect("socket carve declined")
                .body;
            let mated = boolean(&sunk, &ball, BoolOp::Union, 1e-7)
                .expect("ball-in-socket declined")
                .body;
            (mated.worker_mesh(), "ball seated (on-on union)".to_string())
        };
        write_frame(dir, i, &mesh, &format!("\"label\":\"{label}\""));
    }
    println!("socket: {frames} frames");
}

/// The Steinmetz pair (task 29): two equal perpendicular cylinders,
/// the op cycling intersection -> union -> difference (12 frames each).
/// The intersection is the classic bicylinder, volume EXACTLY 16/3.
fn steinmetz(dir: &Path) {
    let frames = 36usize;
    for i in 0..frames {
        let seg = i / 12;
        let mut a = Body::new();
        a.cylinder(
            Frame3::from_z(Vec3::new(0.0, 0.0, -2.0), Vec3::new(0.0, 0.0, 1.0)).unwrap(),
            1.0,
            4.0,
        )
        .unwrap();
        let mut b = Body::new();
        b.cylinder(
            Frame3::from_z(Vec3::new(-2.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0)).unwrap(),
            1.0,
            4.0,
        )
        .unwrap();
        let (op, name) = match seg {
            0 => (BoolOp::Intersection, "intersection = 16/3 exactly"),
            1 => (BoolOp::Union, "union"),
            _ => (BoolOp::Difference, "difference"),
        };
        // Honest-output rule: a declining frame shows the two INPUT
        // bodies and says so (and the decline goes on the bug list).
        match boolean(&a, &b, op, 1e-7) {
            Ok(r) => {
                let mesh = r.body.worker_mesh();
                write_frame(dir, i, &mesh, &format!("\"label\":\"{name}\""));
            }
            Err(_) => {
                let mut mesh = a.worker_mesh();
                let bm = b.worker_mesh();
                let base = (mesh.positions.len() / 3) as u32;
                mesh.positions.extend_from_slice(&bm.positions);
                mesh.normals.extend_from_slice(&bm.normals);
                mesh.indices.extend(bm.indices.iter().map(|&k| k + base));
                mesh.lines.extend_from_slice(&bm.lines);
                write_frame(dir, i, &mesh, "\"declined\":true");
            }
        }
    }
    println!("steinmetz: {frames} frames");
}

/// The countersink family (task 30): a cone frustum sinks deeper into
/// the plate (every frame a real difference), then the EXACT mated plug
/// re-unions (coincident cone laterals).
fn countersink(dir: &Path) {
    let frames = 36usize;
    for i in 0..frames {
        let plug_at = 26usize;
        let t = (i.min(plug_at - 1)) as f64 / (plug_at - 1) as f64;
        let base = 1.9 - 1.4 * t; // frustum base z sinks 1.9 -> 0.5
        let mut block = Body::new();
        block.block(Vec3::ZERO, 4.0, 4.0, 2.0).unwrap();
        let f = Frame3::from_z(Vec3::new(2.0, 2.0, base), Vec3::new(0.0, 0.0, 1.0)).unwrap();
        let mut tool = Body::new();
        tool.loft_circles(f.clone(), 0.4, 0.4 + (3.0 - base), 3.0)
            .unwrap();
        let sunk = boolean(&block, &tool, BoolOp::Difference, 1e-7)
            .expect("countersink frame declined")
            .body;
        let (body, label) = if i < plug_at {
            (sunk, format!("countersink depth {:.2}", 2.0 - base))
        } else {
            let mut plug = Body::new();
            plug.loft_circles(f, 0.4, 0.4 + (3.0 - base), 3.0).unwrap();
            let mated = boolean(&sunk, &plug, BoolOp::Union, 1e-7)
                .expect("plug frame declined")
                .body;
            (mated, "exact mated plug".to_string())
        };
        let mesh = body.worker_mesh();
        write_frame(dir, i, &mesh, &format!("\"label\":\"{label}\""));
    }
    println!("countersink: {frames} frames");
}

/// Primitive parade: torus (minor radius breathing), tube (inner
/// radius breathing), cone (apex height breathing): 12 frames each,
/// every body a real constructor output.
fn primitives(dir: &Path) {
    let frames = 36usize;
    for i in 0..frames {
        let seg = i / 12;
        let t = (i % 12) as f64 / 12.0;
        let w = 1.0 - (2.0 * t - 1.0).abs();
        let f = Frame3::from_z(Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0)).unwrap();
        let (body, label) = match seg {
            0 => {
                let minor = 0.25 + 0.55 * w;
                let mut b = Body::new();
                b.torus(f, 1.5, minor).expect("torus frame");
                (b, format!("torus minor {minor:.2}"))
            }
            1 => {
                let ri = 0.3 + 0.8 * w;
                let mut b = Body::new();
                b.tube(f, ri, 1.5, 1.6).expect("tube frame");
                (b, format!("tube inner {ri:.2}"))
            }
            _ => {
                let h = 1.2 + 1.6 * w;
                let mut b = Body::new();
                b.cone(f, 1.2, h).expect("cone frame");
                (b, format!("cone h {h:.2}"))
            }
        };
        let mesh = body.worker_mesh();
        write_frame(dir, i, &mesh, &format!("\"label\":\"{label}\""));
    }
    println!("primitives: {frames} frames");
}

/// Sweep along a helix: a small square profile swept along a helical
/// path whose turn count breathes (every frame a real swept solid).
fn helix(dir: &Path) {
    let frames = 36usize;
    for i in 0..frames {
        let t = i as f64 / frames as f64;
        let w = 1.0 - (2.0 * t - 1.0).abs();
        let turns = 0.75 + 1.5 * w;
        let n = 48usize;
        let path: Vec<Vec3> = (0..=n)
            .map(|k| {
                let s = turns * core::f64::consts::TAU * k as f64 / n as f64;
                Vec3::new(1.2 * s.cos(), 1.2 * s.sin(), 0.35 * s)
            })
            .collect();
        let profile = [
            Vec3::new(-0.18, -0.18, 0.0),
            Vec3::new(0.18, -0.18, 0.0),
            Vec3::new(0.18, 0.18, 0.0),
            Vec3::new(-0.18, 0.18, 0.0),
        ];
        let mut b = Body::new();
        b.sweep_along_path(&profile, &path).expect("helix frame");
        let mesh = b.worker_mesh();
        write_frame(
            dir,
            i,
            &mesh,
            &format!("\"label\":\"helix sweep {turns:.2} turns\""),
        );
    }
    println!("helix: {frames} frames");
}

/// Variable-radius fillet: one block edge blended r0 -> r1 with r1
/// breathing (every frame a real variable blend).
fn vfillet(dir: &Path) {
    use keel_topo::entity::EdgeKey;
    let frames = 36usize;
    for i in 0..frames {
        let t = i as f64 / frames as f64;
        let w = 1.0 - (2.0 * t - 1.0).abs();
        let r1 = 0.15 + 0.75 * w;
        let mut b = Body::new();
        b.block(Vec3::ZERO, 4.0, 1.6, 2.0).unwrap();
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
        let body = b.fillet_edge_variable(e, 0.15, r1).expect("vfillet frame");
        let mesh = body.worker_mesh();
        write_frame(
            dir,
            i,
            &mesh,
            &format!("\"label\":\"variable fillet 0.15 -> {r1:.2}\""),
        );
    }
    println!("vfillet: {frames} frames");
}

/// Pick the face containing boundary point `p` (pick_face is the
/// kernel's own picking layer).
fn face_near(b: &Body, p: Vec3) -> keel_topo::entity::FaceKey {
    b.pick_face(p, 1e-6).expect("face_near: no face at point")
}

/// Draft: a block's side wall tilts about the bottom neutral plane,
/// angle breathing (every frame a real draft_face).
fn draft(dir: &Path) {
    use keel_geom::surface::Plane3;
    let frames = 36usize;
    for i in 0..frames {
        let t = i as f64 / frames as f64;
        let w = 1.0 - (2.0 * t - 1.0).abs();
        let ang = 0.35 * w;
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.5, 2.5, 2.5).unwrap();
        let f = face_near(&b, Vec3::new(2.5, 1.25, 1.25));
        let neutral = Plane3 {
            frame: Frame3::from_z(Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0)).unwrap(),
        };
        b.draft_face(f, &neutral, ang).expect("draft frame");
        let mesh = b.worker_mesh();
        write_frame(
            dir,
            i,
            &mesh,
            &format!("\"label\":\"draft {:.1} deg\"", ang.to_degrees()),
        );
    }
    println!("draft: {frames} frames");
}

/// Move-face: a boss face slides along the top of a plate and back
/// (every frame a real imprint + push + move_face chain is overkill;
/// the demo moves the top face of a boss block unioned on a plate).
fn moveface(dir: &Path) {
    let frames = 36usize;
    for i in 0..frames {
        let t = i as f64 / frames as f64;
        let w = 1.0 - (2.0 * t - 1.0).abs();
        let dz = 1.4 * w;
        let mut b = Body::new();
        b.block(Vec3::ZERO, 3.0, 3.0, 1.0).unwrap();
        let top = face_near(&b, Vec3::new(1.5, 1.5, 1.0));
        b.move_face(top, Vec3::new(0.0, 0.0, dz))
            .expect("move frame");
        let mesh = b.worker_mesh();
        write_frame(
            dir,
            i,
            &mesh,
            &format!("\"label\":\"move_face dz {dz:.2}\""),
        );
    }
    println!("moveface: {frames} frames");
}

/// Non-uniform scale (task 43, the OCCT-WASM differentiator): a drilled
/// plate squashes and stretches so its ROUND bore becomes a true
/// ELLIPSE. The exact rational route maps the cylinder bore to its
/// elliptic-cylinder NURBS image and the rim circles to ellipses; every
/// frame is a real scaled body off one drilled plate.
fn nonuniform(dir: &Path) {
    let frames = 36usize;
    let mut plate = Body::new();
    plate.block(Vec3::ZERO, 4.0, 4.0, 1.0).unwrap();
    let bf = Frame3::from_z(Vec3::new(2.0, 2.0, -0.5), Vec3::new(0.0, 0.0, 1.0)).unwrap();
    let mut tool = Body::new();
    tool.cylinder(bf, 1.0, 2.0).unwrap();
    let drilled = boolean(&plate, &tool, BoolOp::Difference, 1e-7)
        .expect("nonuniform drill declined")
        .body;
    for i in 0..frames {
        let t = i as f64 / frames as f64;
        let w = (core::f64::consts::TAU * t).sin();
        let sx = 1.0 + 0.5 * w;
        let scaled = drilled
            .scaled_nonuniform(Vec3::new(2.0, 2.0, 0.5), Vec3::new(sx, 1.0 / sx, 1.0))
            .expect("nonuniform frame");
        let mesh = scaled.worker_mesh();
        write_frame(
            dir,
            i,
            &mesh,
            &format!("\"label\":\"bore -> ellipse, sx {sx:.2}\""),
        );
    }
    println!("nonuniform: {frames} frames");
}

/// Partial revolve: a vase profile revolved through a breathing sweep
/// angle (every frame a real revolve_partial).
fn revolve_partial_demo(dir: &Path) {
    let frames = 36usize;
    for i in 0..frames {
        let t = i as f64 / frames as f64;
        let w = 1.0 - (2.0 * t - 1.0).abs();
        let theta = 0.7 + (core::f64::consts::TAU - 1.4) * w;
        let f = Frame3::from_z(Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0)).unwrap();
        let mut b = Body::new();
        b.revolve_partial(
            f.clone(),
            &[(0.4, 0.0), (1.2, 0.0), (0.9, 0.8), (1.1, 1.6), (0.4, 1.6)],
            theta,
        )
        .expect("revolve_partial frame");
        let mesh = b.worker_mesh();
        write_frame(
            dir,
            i,
            &mesh,
            &format!("\"label\":\"revolve {:.0} deg\"", theta.to_degrees()),
        );
    }
    println!("revolve_partial: {frames} frames");
}

/// Multi-section loft: a square-to-circle-to-square tower whose middle
/// section breathes (every frame a real loft_sections).
fn loftdemo(dir: &Path) {
    let frames = 36usize;
    for i in 0..frames {
        let t = i as f64 / frames as f64;
        let w = 1.0 - (2.0 * t - 1.0).abs();
        let rmid = 0.55 + 0.75 * w;
        // Same-shape scaled sections (the planar-side-quad contract:
        // corresponding chords stay parallel, so each side quad is a
        // planar trapezoid; shape-morphing sections decline honestly).
        let n = 12usize;
        let ring = |r: f64, z: f64| -> Vec<Vec3> {
            (0..n)
                .map(|k| {
                    let a = core::f64::consts::TAU * k as f64 / n as f64;
                    Vec3::new(r * a.cos(), r * a.sin(), z)
                })
                .collect()
        };
        let s0 = ring(1.0, 0.0);
        let s1 = ring(rmid, 1.1);
        let s2 = ring(0.8, 2.2);
        let mut b = Body::new();
        b.loft_sections(&[&s0, &s1, &s2]).expect("loft frame");
        let mesh = b.worker_mesh();
        write_frame(
            dir,
            i,
            &mesh,
            &format!("\"label\":\"loft mid r {rmid:.2}\""),
        );
    }
    println!("loft: {frames} frames");
}

/// Wire-solid boolean: a segment sweeps through a cube; intersection
/// keeps the inside length, difference the outside tails (drawn as
/// edges over the glass cube).
fn wiretrim(dir: &Path) {
    use keel_topo::boolean::boolean_wire_solid;
    let frames = 36usize;
    for i in 0..frames {
        let t = i as f64 / frames as f64;
        let y = 0.2 + 1.6 * (1.0 - (2.0 * t - 1.0).abs());
        let mut cube = Body::new();
        cube.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        let mut w = Body::new();
        w.wire(Vec3::new(-1.0, y, 1.0), Vec3::new(3.0, y, 1.4))
            .unwrap();
        let op = if i % 2 == 0 {
            BoolOp::Intersection
        } else {
            BoolOp::Difference
        };
        let trimmed = boolean_wire_solid(&w, &cube, op, 1e-7).expect("wire frame");
        // Show the cube (glass) plus the trimmed wire as lines.
        let mut mesh = cube.worker_mesh();
        let wm = trimmed.worker_mesh();
        mesh.lines.extend_from_slice(&wm.lines);
        let name = if i % 2 == 0 {
            "wire inside"
        } else {
            "wire outside"
        };
        write_frame(dir, i, &mesh, &format!("\"label\":\"{name}\""));
    }
    println!("wiretrim: {frames} frames");
}

/// Find a block edge by its midpoint.
fn edge_at(b: &Body, mid: Vec3) -> keel_topo::entity::EdgeKey {
    b.entity_ids()
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
            ((p0 + p1) * 0.5 - mid).norm() < 1e-9
        })
        .expect("edge_at: no edge with that midpoint")
}

/// The blend zoo: the same block edge wearing each advanced blend in
/// turn (g2, conic, hold-line, partial extent, chain, notch): 6 frames
/// each, every body a real blend.
fn blendzoo(dir: &Path) {
    let frames = 36usize;
    for i in 0..frames {
        let seg = i / 6;
        let t = (i % 6) as f64 / 6.0;
        let w = 1.0 - (2.0 * t - 1.0).abs();
        let mut b = Body::new();
        b.block(Vec3::ZERO, 4.0, 1.6, 2.0).unwrap();
        let e = edge_at(&b, Vec3::new(2.0, 0.0, 2.0));
        let (body, label) = match seg {
            0 => {
                // fullness contract: (0, 0.5].
                let full = 0.12 + 0.36 * w;
                (
                    b.fillet_edge_g2(e, 0.5, full).expect("g2 frame"),
                    format!("G2 blend fullness {full:.2}"),
                )
            }
            1 => {
                let cw = 0.3 + 0.6 * w;
                (
                    b.fillet_edge_conic(e, 0.5, cw).expect("conic frame"),
                    format!("conic blend rho {cw:.2}"),
                )
            }
            2 => {
                let s = 0.25 + 0.5 * w;
                (
                    b.fillet_edge_hold_line(e, true, s).expect("hold frame"),
                    format!("hold-line setback {s:.2}"),
                )
            }
            3 => {
                let half = 0.15 + 0.2 * w;
                (
                    b.fillet_edge_partial(e, 0.5 - half, 0.5 + half, 0.45)
                        .expect("partial frame"),
                    "partial-extent fillet".to_string(),
                )
            }
            4 => {
                // Roll-on chain: the union of two abutting cubes splits
                // the top edge into two COLLINEAR pieces (the op's
                // contract); the blend rolls across both.
                let mut ua = Body::new();
                ua.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
                let mut ub = Body::new();
                ub.block(Vec3::new(2.0, 0.0, 0.0), 2.0, 2.0, 2.0).unwrap();
                let split = boolean(&ua, &ub, BoolOp::Union, 1e-7)
                    .expect("chain union")
                    .body;
                let e1 = edge_at(&split, Vec3::new(1.0, 2.0, 2.0));
                let e2 = edge_at(&split, Vec3::new(3.0, 2.0, 2.0));
                let r = 0.25 + 0.3 * w;
                (
                    split.fillet_edge_chain(e1, e2, r).expect("chain frame"),
                    format!("chain roll-on r {r:.2}"),
                )
            }
            _ => {
                // Notch: a groove crosses the blend corridor; the
                // ribbon bridges it undeformed.
                let mut bb = Body::new();
                bb.block(Vec3::ZERO, 4.0, 2.0, 2.0).unwrap();
                let mut tool = Body::new();
                tool.block(Vec3::new(1.9, -0.5, 1.8), 0.2, 3.0, 1.0)
                    .unwrap();
                let g = boolean(&bb, &tool, BoolOp::Difference, 1e-7)
                    .expect("notch groove")
                    .body;
                let e1 = edge_at(&g, Vec3::new(0.95, 2.0, 2.0));
                let e2 = edge_at(&g, Vec3::new(3.05, 2.0, 2.0));
                let r = 0.3 + 0.25 * w;
                (
                    g.fillet_edge_notch(e1, e2, r).expect("notch frame"),
                    format!("notch blend r {r:.2}"),
                )
            }
        };
        let mesh = body.worker_mesh();
        write_frame(dir, i, &mesh, &format!("\"label\":\"{label}\""));
    }
    println!("blendzoo: {frames} frames");
}

/// The corner family beyond the octant: the mitre corner (two edges)
/// and the setback corner (three edges, per-edge radius + setback),
/// radius breathing.
fn corners2(dir: &Path) {
    let frames = 36usize;
    for i in 0..frames {
        let half = 18usize;
        let t = (i % half) as f64 / half as f64;
        let w = 1.0 - (2.0 * t - 1.0).abs();
        let r = 0.2 + 0.35 * w;
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        let (body, label) = if i < half {
            let e1 = edge_at(&b, Vec3::new(1.0, 0.0, 2.0));
            let e2 = edge_at(&b, Vec3::new(0.0, 1.0, 2.0));
            (
                b.mitre_fillet_corner(e1, e2, r).expect("mitre frame"),
                format!("mitre corner r {r:.2}"),
            )
        } else {
            let e1 = edge_at(&b, Vec3::new(1.0, 2.0, 2.0));
            let e2 = edge_at(&b, Vec3::new(2.0, 1.0, 2.0));
            let e3 = edge_at(&b, Vec3::new(2.0, 2.0, 1.0));
            (
                b.fillet_corner_setback(&[(e1, r, 0.6), (e2, r, 0.6), (e3, r, 0.6)])
                    .expect("setback frame"),
                format!("setback corner r {r:.2}"),
            )
        };
        let mesh = body.worker_mesh();
        write_frame(dir, i, &mesh, &format!("\"label\":\"{label}\""));
    }
    println!("corners2: {frames} frames");
}

/// Unblend: a fillet appears, then blend RECOGNITION removes it (the
/// defeature path): alternating blended / recovered frames.
fn unblend(dir: &Path) {
    let frames = 36usize;
    for i in 0..frames {
        let t = i as f64 / frames as f64;
        let w = 1.0 - (2.0 * t - 1.0).abs();
        let r = 0.25 + 0.45 * w;
        let mut b = Body::new();
        b.block(Vec3::ZERO, 4.0, 1.6, 2.0).unwrap();
        let e = edge_at(&b, Vec3::new(2.0, 0.0, 2.0));
        let mut blended = b.fillet_edge(e, r).expect("unblend fillet frame");
        let recovered = i % 12 >= 6;
        let label = if recovered {
            let (removed, remaining) = blended.unblend_all(1e-6);
            format!("unblend: {removed} removed, {remaining} left")
        } else {
            format!("fillet r {r:.2}")
        };
        let mesh = blended.worker_mesh();
        write_frame(dir, i, &mesh, &format!("\"label\":\"{label}\""));
    }
    println!("unblend: {frames} frames");
}

/// Sheet ops: a planar sheet trims by a sweeping plane, then thickens
/// into a solid plate (both real sheet operations).
fn sheets(dir: &Path) {
    let frames = 36usize;
    for i in 0..frames {
        let half = 18usize;
        let profile = [
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(3.0, 0.0, 0.6),
            Vec3::new(3.0, 2.4, 0.6),
            Vec3::new(0.0, 2.4, 0.0),
        ];
        let sheet = Body::planar_sheet(&profile).expect("sheet");
        let (body, label) = if i < half {
            let t = i as f64 / half as f64;
            let x = 0.4 + 2.4 * (1.0 - (2.0 * t - 1.0).abs());
            let trimmed = sheet
                .trim_by_plane(Vec3::new(x, 1.2, 0.3), Vec3::new(-1.0, 0.0, 0.0))
                .expect("trim frame");
            (trimmed, format!("sheet trim x {x:.2}"))
        } else {
            let t = (i - half) as f64 / half as f64;
            let th = 0.1 + 0.5 * (1.0 - (2.0 * t - 1.0).abs());
            let thick = sheet.thicken(th).expect("thicken frame");
            (thick, format!("thicken t {th:.2}"))
        };
        let mesh = body.worker_mesh();
        write_frame(dir, i, &mesh, &format!("\"label\":\"{label}\""));
    }
    println!("sheets: {frames} frames");
}

/// Partition by sheet: a knife sheet sweeps through a block; the
/// cellular result keeps the interior wall (drawn through the glass),
/// then merge_cells dissolves it back.
fn partition(dir: &Path) {
    use keel_topo::boolean::partition_by_sheet;
    let frames = 36usize;
    for i in 0..frames {
        let t = i as f64 / frames as f64;
        let ang = 0.5 * (core::f64::consts::TAU * t).sin();
        let n = Vec3::new(ang.sin(), 0.0, ang.cos());
        let u = Vec3::new(ang.cos(), 0.0, -ang.sin());
        // rectangular_sheet takes the CORNER: center the 6x6 knife on
        // the block centre.
        let v = n.cross(u);
        let corner = Vec3::new(1.5, 1.5, 1.0) - u * 3.0 - v * 3.0;
        let knife = Body::rectangular_sheet(corner, n, u, 6.0, 6.0).expect("knife sheet");
        let mut block = Body::new();
        block.block(Vec3::ZERO, 3.0, 3.0, 2.0).unwrap();
        // Honest-output: a declining tilt shows the inputs + the flag
        // (and goes on the bug list: see the task notes).
        match partition_by_sheet(&block, &knife, 1e-7) {
            Ok(body) => {
                let mesh = body.worker_mesh();
                write_frame(
                    dir,
                    i,
                    &mesh,
                    &format!("\"label\":\"partition tilt {:.0} deg\"", ang.to_degrees()),
                );
            }
            Err(e) => {
                eprintln!("  partition frame {i} tilt {:.3} declined: {e:?}", ang);
                let mut mesh = block.worker_mesh();
                let km = knife.worker_mesh();
                let base = (mesh.positions.len() / 3) as u32;
                mesh.positions.extend_from_slice(&km.positions);
                mesh.normals.extend_from_slice(&km.normals);
                mesh.indices.extend(km.indices.iter().map(|&k| k + base));
                mesh.lines.extend_from_slice(&km.lines);
                write_frame(dir, i, &mesh, "\"declined\":true");
            }
        }
    }
    println!("partition: {frames} frames");
}

/// Multi-tool boolean: a plate drilled by a ring of cylinders in ONE
/// boolean_multi call, the ring radius breathing.
fn multitool(dir: &Path) {
    use keel_topo::boolean::boolean_multi;
    let frames = 36usize;
    for i in 0..frames {
        let t = i as f64 / frames as f64;
        let w = 1.0 - (2.0 * t - 1.0).abs();
        let ring = 0.9 + 0.5 * w;
        let mut plate = Body::new();
        plate.block(Vec3::ZERO, 4.0, 4.0, 0.8).unwrap();
        let tools: Vec<Body> = (0..5)
            .map(|k| {
                let a = core::f64::consts::TAU * k as f64 / 5.0;
                let f = Frame3::from_z(
                    Vec3::new(2.0 + ring * a.cos(), 2.0 + ring * a.sin(), -0.5),
                    Vec3::new(0.0, 0.0, 1.0),
                )
                .unwrap();
                let mut c = Body::new();
                c.cylinder(f, 0.28, 2.0).unwrap();
                c
            })
            .collect();
        let refs: Vec<&Body> = tools.iter().collect();
        let body = boolean_multi(&plate, &refs, BoolOp::Difference, 1e-7)
            .expect("multitool frame")
            .body;
        let mesh = body.worker_mesh();
        write_frame(
            dir,
            i,
            &mesh,
            &format!("\"label\":\"5 tools, one call (ring {ring:.2})\""),
        );
    }
    println!("multitool: {frames} frames");
}

/// Slice stack: a vase sliced into wafers by parallel planes, the
/// wafer count breathing (every frame real `slice` output, wafers
/// shown exploded along z).
fn slicestack(dir: &Path) {
    let frames = 36usize;
    for i in 0..frames {
        let t = i as f64 / frames as f64;
        let w = 1.0 - (2.0 * t - 1.0).abs();
        let gap = 0.05 + 0.45 * w;
        let f = Frame3::from_z(Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0)).unwrap();
        let mut vase = Body::new();
        vase.revolve(
            f,
            &[(0.0, 0.0), (1.0, 0.0), (0.7, 0.8), (0.9, 1.6), (0.0, 1.6)],
        )
        .expect("slice vase");
        let offsets: Vec<f64> = (1..4).map(|k| k as f64 * 0.4).collect();
        let pieces = vase
            .slice(Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0), &offsets)
            .expect("slice frame");
        // Explode the wafers along z and merge into one frame mesh.
        let mut mesh = WorkerMesh::default();
        for (k, p) in pieces.iter().enumerate() {
            let lifted = p
                .transformed(&keel_math::transform::Transform3::from_translation(
                    Vec3::new(0.0, 0.0, k as f64 * gap),
                ))
                .expect("lift wafer");
            let pm = lifted.worker_mesh();
            let base = (mesh.positions.len() / 3) as u32;
            mesh.positions.extend_from_slice(&pm.positions);
            mesh.normals.extend_from_slice(&pm.normals);
            mesh.indices.extend(pm.indices.iter().map(|&x| x + base));
            mesh.lines.extend_from_slice(&pm.lines);
        }
        write_frame(
            dir,
            i,
            &mesh,
            &format!("\"label\":\"slice x{} gap {gap:.2}\"", offsets.len() + 1),
        );
    }
    println!("slicestack: {frames} frames");
}

/// Defeature: a plate with a grid of small drilled holes; the
/// defeature threshold sweeps and holes vanish (real
/// defeature_small_holes on a really drilled plate).
fn defeature(dir: &Path) {
    use keel_topo::boolean::boolean_multi;
    let frames = 36usize;
    for i in 0..frames {
        let t = i as f64 / frames as f64;
        let w = 1.0 - (2.0 * t - 1.0).abs();
        let max_area = 0.02 + 1.4 * w;
        let mut plate = Body::new();
        plate.block(Vec3::ZERO, 4.0, 4.0, 0.6).unwrap();
        let radii = [0.12, 0.22, 0.34, 0.5];
        let tools: Vec<Body> = radii
            .iter()
            .enumerate()
            .map(|(k, &r)| {
                let f = Frame3::from_z(
                    Vec3::new(0.8 + 0.8 * k as f64, 0.8 + 0.8 * k as f64, -0.5),
                    Vec3::new(0.0, 0.0, 1.0),
                )
                .unwrap();
                let mut c = Body::new();
                c.cylinder(f, r, 2.0).unwrap();
                c
            })
            .collect();
        let refs: Vec<&Body> = tools.iter().collect();
        let mut body = boolean_multi(&plate, &refs, BoolOp::Difference, 1e-7)
            .expect("defeature drill")
            .body;
        let n = body
            .defeature_small_holes(max_area)
            .expect("defeature frame");
        let mesh = body.worker_mesh();
        write_frame(
            dir,
            i,
            &mesh,
            &format!("\"label\":\"defeature < {max_area:.2}: {n} healed\""),
        );
    }
    println!("defeature: {frames} frames");
}

/// Hollow with a pierced top: the open-box shell, wall thickness
/// breathing (real hollow_pierce per frame).
fn pierce(dir: &Path) {
    let frames = 36usize;
    for i in 0..frames {
        let t = i as f64 / frames as f64;
        let w = 1.0 - (2.0 * t - 1.0).abs();
        let th = 0.1 + 0.4 * w;
        let mut b = Body::new();
        b.block(Vec3::ZERO, 3.0, 2.2, 1.6).unwrap();
        let top = face_near(&b, Vec3::new(1.5, 1.1, 1.6));
        let body = b.hollow_pierce(th, |f| f == top).expect("pierce frame");
        let mesh = body.worker_mesh();
        write_frame(dir, i, &mesh, &format!("\"label\":\"open box t {th:.2}\""));
    }
    println!("pierce: {frames} frames");
}

/// Mirror + transform: an L-bracket and its mirror image drifting
/// apart and back (real mirrored/transformed bodies, knitted view).
fn mirror(dir: &Path) {
    let frames = 36usize;
    for i in 0..frames {
        let t = i as f64 / frames as f64;
        let gap = 0.2 + 1.2 * (1.0 - (2.0 * t - 1.0).abs());
        let base: Vec<Vec3> = [
            (0.0, 0.0),
            (2.0, 0.0),
            (2.0, 0.7),
            (0.7, 0.7),
            (0.7, 2.0),
            (0.0, 2.0),
        ]
        .iter()
        .map(|&(x, y)| Vec3::new(x, y, 0.0))
        .collect();
        let mut b = Body::new();
        b.prism(&base, Vec3::new(0.0, 0.0, 0.8))
            .expect("mirror prism");
        let left = b
            .transformed(&keel_math::transform::Transform3::from_translation(
                Vec3::new(-gap, 0.0, 0.0),
            ))
            .expect("mirror move");
        let right = left
            .mirrored(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0))
            .expect("mirror frame");
        let mut mesh = left.worker_mesh();
        let rm = right.worker_mesh();
        let basei = (mesh.positions.len() / 3) as u32;
        mesh.positions.extend_from_slice(&rm.positions);
        mesh.normals.extend_from_slice(&rm.normals);
        mesh.indices.extend(rm.indices.iter().map(|&k| k + basei));
        mesh.lines.extend_from_slice(&rm.lines);
        write_frame(dir, i, &mesh, "\"label\":\"mirrored pair\"");
    }
    println!("mirror: {frames} frames");
}

/// Taper-face: a block wall rotates about a pivot axis (real
/// taper_face per frame), the complement of draft_face's neutral-plane
/// form.
fn taperface(dir: &Path) {
    let frames = 36usize;
    for i in 0..frames {
        let t = i as f64 / frames as f64;
        let ang = 0.30 * (core::f64::consts::TAU * t).sin();
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.5, 2.5, 2.5).unwrap();
        let f = face_near(&b, Vec3::new(2.5, 1.25, 1.25));
        b.taper_face(f, Vec3::new(2.5, 1.25, 0.0), Vec3::new(0.0, 1.0, 0.0), ang)
            .expect("taper frame");
        let mesh = b.worker_mesh();
        write_frame(
            dir,
            i,
            &mesh,
            &format!("\"label\":\"taper {:.1} deg\"", ang.to_degrees()),
        );
    }
    println!("taperface: {frames} frames");
}

/// Delete-face (the COPLANAR-merge heal, item 40): the union of two
/// abutting cubes leaves every transverse face split at the joint;
/// delete_face dissolves one split half into its coplanar neighbour
/// (the joint line vanishes). The second cube's height offset breathes
/// so the joint is visible when present.
fn deleteface(dir: &Path) {
    let frames = 36usize;
    for i in 0..frames {
        let healed = i % 12 >= 6;
        let mut ua = Body::new();
        ua.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        let mut ub = Body::new();
        ub.block(Vec3::new(2.0, 0.0, 0.0), 2.0, 2.0, 2.0).unwrap();
        let mut body = boolean(&ua, &ub, BoolOp::Union, 1e-7)
            .expect("deleteface union")
            .body;
        let label = if healed {
            let half = face_near(&body, Vec3::new(1.0, 1.0, 2.0));
            match body.delete_face(half) {
                Ok(()) => "delete_face: coplanar split healed".to_string(),
                Err(_) => "delete_face declined".to_string(),
            }
        } else {
            "union: top face split at the joint".to_string()
        };
        let mesh = body.worker_mesh();
        write_frame(dir, i, &mesh, &format!("\"label\":\"{label}\""));
    }
    println!("deleteface: {frames} frames");
}

/// Knit: four open sheets fold up into a closed solid tetrahedron-ish
/// pocket: per frame the dihedral angle breathes; the final third
/// knits the sheets into one solid body.
fn knitdemo(dir: &Path) {
    use keel_topo::boolean::knit;
    let frames = 36usize;
    for i in 0..frames {
        let t = i as f64 / frames as f64;
        let h = 0.6 + 2.0 * (1.0 - (2.0 * t - 1.0).abs());
        // A square base sheet and four triangular side sheets sharing
        // ONE apex: five separate open sheets that knit into a closed
        // solid pyramid.
        let s = 1.4;
        let apex = Vec3::new(0.0, 0.0, h);
        let c = [
            Vec3::new(-s, -s, 0.0),
            Vec3::new(s, -s, 0.0),
            Vec3::new(s, s, 0.0),
            Vec3::new(-s, s, 0.0),
        ];
        let base = Body::planar_sheet(&[c[0], c[1], c[2], c[3]]).expect("base");
        let flaps: Vec<Body> = (0..4)
            .map(|k| {
                let a = c[k];
                let b2 = c[(k + 1) % 4];
                Body::planar_sheet(&[a, b2, apex]).expect("flap")
            })
            .collect();
        let mut all: Vec<&Body> = vec![&base];
        all.extend(flaps.iter());
        match knit(&all, 1e-7) {
            Ok(body) => {
                let solidity = if body.epsilon_solid(1e-6) {
                    "solid"
                } else {
                    "open"
                };
                let mesh = body.worker_mesh();
                write_frame(dir, i, &mesh, &format!("\"label\":\"knit: {solidity}\""));
            }
            Err(_) => {
                let mut mesh = base.worker_mesh();
                for f in &flaps {
                    let fm = f.worker_mesh();
                    let basei = (mesh.positions.len() / 3) as u32;
                    mesh.positions.extend_from_slice(&fm.positions);
                    mesh.normals.extend_from_slice(&fm.normals);
                    mesh.indices.extend(fm.indices.iter().map(|&k| k + basei));
                    mesh.lines.extend_from_slice(&fm.lines);
                }
                write_frame(dir, i, &mesh, "\"label\":\"sheets (knit declined)\"");
            }
        }
    }
    println!("knit: {frames} frames");
}

/// Hidden-line wireframe: the drilled plate drawn ONLY through the
/// kernel's HLR (visible lines bright, no facets): the wireframe is
/// the product under test.
fn hlr(dir: &Path) {
    let frames = 36usize;
    for i in 0..frames {
        let t = i as f64 / frames as f64;
        let w = 1.0 - (2.0 * t - 1.0).abs();
        let r = 0.4 + 0.9 * w;
        let mut plate = Body::new();
        plate.block(Vec3::ZERO, 4.0, 4.0, 1.2).unwrap();
        let f = Frame3::from_z(Vec3::new(2.0, 2.0, -0.5), Vec3::new(0.0, 0.0, 1.0)).unwrap();
        let mut tool = Body::new();
        tool.cylinder(f, r, 2.5).unwrap();
        let body = boolean(&plate, &tool, BoolOp::Difference, 1e-7)
            .expect("hlr drill")
            .body;
        let view = Vec3::new(0.55, -0.55, 0.63);
        let wf = body.hidden_line_wireframe(view);
        let mut mesh = WorkerMesh::default();
        for seg in wf.visible {
            for p in seg {
                mesh.lines
                    .extend_from_slice(&[p.x as f32, p.y as f32, p.z as f32]);
            }
        }
        write_frame(
            dir,
            i,
            &mesh,
            &format!("\"label\":\"hidden-line wireframe r {r:.2}\""),
        );
    }
    println!("hlr: {frames} frames");
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
        "corner" => corner(dir),
        "cellular" => cellular(dir),
        "decline" => decline(dir),
        "construct" => construct(dir),
        "shell" => shell(dir),
        "offset" => offset(dir),
        "gwn" => gwn(dir),
        "imprint_push" => imprint_push(dir),
        "merge" => merge(dir),
        "socket" => socket(dir),
        "steinmetz" => steinmetz(dir),
        "countersink" => countersink(dir),
        "primitives" => primitives(dir),
        "helix" => helix(dir),
        "vfillet" => vfillet(dir),
        "draft" => draft(dir),
        "moveface" => moveface(dir),
        "nonuniform" => nonuniform(dir),
        "revolve_partial" => revolve_partial_demo(dir),
        "loft" => loftdemo(dir),
        "wiretrim" => wiretrim(dir),
        "blendzoo" => blendzoo(dir),
        "corners2" => corners2(dir),
        "unblend" => unblend(dir),
        "sheets" => sheets(dir),
        "partition" => partition(dir),
        "multitool" => multitool(dir),
        "slicestack" => slicestack(dir),
        "defeature" => defeature(dir),
        "pierce" => pierce(dir),
        "mirror" => mirror(dir),
        "taperface" => taperface(dir),
        "deleteface" => deleteface(dir),
        "knit" => knitdemo(dir),
        "hlr" => hlr(dir),
        other => {
            eprintln!("unknown op {other}; available: drill, trio, pin, fillet, corner");
            std::process::exit(1);
        }
    }
}
