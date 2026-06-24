//! worker_mesh() must emit a WATERTIGHT render mesh: for any CLOSED solid,
//! the triangle soup has ZERO boundary edges once vertices are welded by
//! position. A boundary edge is a directed half-edge (u,v) with no opposing
//! (v,u) after welding -- the signature of a T-junction crack where two
//! faces sampled a shared curved rim at different points.
//!
//! Root cause (pre-fix): planar caps sampled a circular rim via loop_polygon
//! (~32/turn from the arc's start vertex) while the cylinder/torus wall
//! sampled it on its own parametric grid (uniform angular, different count
//! and phase), so the rim vertices did not coincide. The fix routes every
//! face's boundary through the SAME per-edge polyline (edge_polyline_opt),
//! so adjacent faces index identical rim points.
//!
//! Hard measurements that motivated the fix (block 2.4x1.6x0.4, bore r0.3):
//! plain bore left 306 unpaired rim edges, bore+rim-fillet left 632, box 0.

use keel_geom::surface::Frame3;
use keel_math::vec::Vec3;
use keel_topo::body::Body;
use keel_topo::boolean::{BoolOp, boolean};
use keel_topo::render::WorkerMesh;

/// Count unpaired directed edges in a welded triangle mesh. Vertices are
/// welded by rounding to a 1e-4 grid (well below any real feature here, well
/// above f32 quantization). A closed oriented manifold has every directed
/// edge (u,v) matched by exactly one (v,u), so the return is 0; any crack
/// leaves an excess of un-mirrored half-edges.
fn boundary_edges(m: &WorkerMesh) -> usize {
    use std::collections::HashMap;
    let q = |x: f32| (x as f64 * 1e4).round() as i64;
    let coord = |i: u32| {
        let k = i as usize * 3;
        (q(m.positions[k]), q(m.positions[k + 1]), q(m.positions[k + 2]))
    };
    let mut ids: HashMap<(i64, i64, i64), u32> = HashMap::new();
    let mut vid = |i: u32| -> u32 {
        let key = coord(i);
        let next = ids.len() as u32;
        *ids.entry(key).or_insert(next)
    };
    let mut dir: HashMap<(u32, u32), i64> = HashMap::new();
    for tri in m.indices.chunks(3) {
        let (a, b, c) = (vid(tri[0]), vid(tri[1]), vid(tri[2]));
        for (u, v) in [(a, b), (b, c), (c, a)] {
            *dir.entry((u, v)).or_insert(0) += 1;
        }
    }
    // Excess half-edges: sum over directed edges of max(0, fwd - bwd).
    let mut bnd = 0usize;
    for (&(u, v), &fwd) in &dir {
        let bwd = dir.get(&(v, u)).copied().unwrap_or(0);
        if fwd > bwd {
            bnd += (fwd - bwd) as usize;
        }
    }
    bnd
}

fn drilled_plate() -> Body {
    let mut plate = Body::new();
    plate.block(Vec3::ZERO, 4.0, 4.0, 1.0).unwrap();
    let dframe = Frame3::from_z(Vec3::new(2.0, 2.0, -0.5), Vec3::new(0.0, 0.0, 1.0)).unwrap();
    let mut drill = Body::new();
    drill.cylinder(dframe, 1.0, 2.0).unwrap();
    boolean(&plate, &drill, BoolOp::Difference, 1e-7).unwrap().body
}

#[test]
fn box_is_watertight() {
    let mut b = Body::new();
    b.block(Vec3::ZERO, 2.0, 1.0, 1.0).unwrap();
    let m = b.worker_mesh();
    let bnd = boundary_edges(&m);
    assert_eq!(bnd, 0, "a closed box mesh has no boundary edges (got {bnd})");
}

#[test]
fn drilled_plate_is_watertight() {
    let body = drilled_plate();
    let bnd = boundary_edges(&body.worker_mesh());
    assert_eq!(
        bnd, 0,
        "drilled plate render mesh must be watertight: {bnd} unpaired rim edges"
    );
    // Chord-independent: the user measured MORE cracks at finer chords (306 -> 484).
    // The conforming fix must hold at any density (both faces consume the same
    // per-edge polyline at the SAME chord).
    for chord in [0.5, 1e-2, 1e-4] {
        let bnd = boundary_edges(&body.worker_mesh_tol(chord));
        assert_eq!(bnd, 0, "drilled plate watertight at chord {chord}: {bnd} unpaired");
    }
}

#[test]
fn drilled_filleted_plate_is_watertight() {
    let body = drilled_plate();
    // Fillet the TOP bore rim (z=1, radius 1 about (2,2)); pick a point on it.
    let pick = Vec3::new(3.0, 2.0, 1.0);
    let e = body
        .nearest_edge(pick)
        .expect("a rim edge near the pick point");
    let filleted = body
        .fillet_edge(e, 0.2)
        .expect("rim fillet must succeed (the fieldforge P0 op chain)");
    let bnd = boundary_edges(&filleted.worker_mesh());
    assert_eq!(
        bnd, 0,
        "drilled+filleted plate render mesh must be watertight: {bnd} unpaired edges"
    );
    // Chord-independent (the user measured cracks growing at finer chords): the
    // conforming torus-fillet must be watertight at coarse draft AND fine final
    // density. Coarse stresses the sparse rim vs denser tube-row stitching.
    for chord in [0.5, 1e-2, 1e-4] {
        let bnd = boundary_edges(&filleted.worker_mesh_tol(chord));
        assert_eq!(bnd, 0, "filleted plate watertight at chord {chord}: {bnd} unpaired");
    }
}
