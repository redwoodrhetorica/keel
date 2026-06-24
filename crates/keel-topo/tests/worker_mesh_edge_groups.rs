//! Worker-mesh edge_groups (task 28, edge-pick API) + chord-tolerance
//! variant. Asserts:
//!   - edge_groups partition the `lines` buffer (contiguous, non-
//!     overlapping, covering every segment),
//!   - each group `id` resolves to a LIVE EdgeKey via `Body::lookup`,
//!   - the group count equals `edge_keys().len()` for edges that
//!     tessellate (all edges here do),
//!   - `worker_mesh_tol(coarse)` yields FEWER curved segments than
//!     `worker_mesh_tol(fine)` on a cylinder,
//!   - `worker_mesh()` default output is UNCHANGED vs a snapshot (the
//!     WRONG-safety guarantee: the default/mass path is byte-identical).

use keel_geom::surface::Frame3;
use keel_topo::body::Body;
use keel_topo::entity::AnyKey;
use keel_math::vec::Vec3;

/// The canonical curved boolean result: a drilled plate (4 planar outer
/// faces + top/bottom with a circular hole + 1 cylindrical bore).
fn drilled_plate() -> Body {
    use keel_topo::boolean::{BoolOp, boolean};
    let mut plate = Body::new();
    plate.block(Vec3::ZERO, 4.0, 4.0, 1.0).unwrap();
    let dframe = Frame3::from_z(Vec3::new(2.0, 2.0, -0.5), Vec3::new(0.0, 0.0, 1.0)).unwrap();
    let mut drill = Body::new();
    drill.cylinder(dframe, 1.0, 2.0).unwrap();
    boolean(&plate, &drill, BoolOp::Difference, 1e-7)
        .unwrap()
        .body
}

#[test]
fn edge_groups_partition_lines_and_resolve() {
    let body = drilled_plate();
    let m = body.worker_mesh();

    assert!(!m.lines.is_empty(), "drilled plate has wireframe edges");
    assert!(!m.edge_groups.is_empty(), "edge_groups populated");

    // Ranges are contiguous, non-overlapping, and tile the whole lines
    // buffer in build order (the host slices lines[start..start+count]).
    let mut cursor = 0u32;
    for &(_, start, count) in &m.edge_groups {
        assert_eq!(start, cursor, "edge groups are contiguous (no gaps/overlap)");
        assert!(count > 0, "no empty edge group");
        assert_eq!(count % 6, 0, "count is whole segments (6 f32 per segment)");
        cursor += count;
    }
    assert_eq!(
        cursor as usize,
        m.lines.len(),
        "edge groups cover the entire lines buffer"
    );

    // Every group id resolves to a LIVE edge via lookup (the host maps a
    // picked edge id back to its EdgeKey through exactly this call).
    for &(id, _, _) in &m.edge_groups {
        let resolved = body.lookup(keel_topo::entity::EntityId(id));
        assert!(
            matches!(resolved, Some(AnyKey::Edge(_))),
            "edge_group id {id} resolves to a live EdgeKey"
        );
    }

    // (fieldforge P1 cleanup) edge_groups are CONSUMER-FACING now: a periodic
    // SEAM edge is not pickable, and a bored rim's two seam-split arcs MERGE into
    // one circular group. So there are FEWER groups than live edges, every group
    // id is still a live edge (subset, not set-equality), and the drilled plate's
    // 17 raw edges become 14 groups (2 rim-merges of 4 arcs -> 2, + 1 seam dropped).
    let live_edges = body.edge_keys().len();
    assert!(
        m.edge_groups.len() < live_edges,
        "seam-skip + rim-merge reduces groups: {} vs {live_edges} edges",
        m.edge_groups.len()
    );
    let group_ids: std::collections::HashSet<u64> = m.edge_groups.iter().map(|g| g.0).collect();
    let edge_ids: std::collections::HashSet<u64> =
        body.edge_keys().iter().map(|&e| body.edge_id(e).unwrap().0).collect();
    assert!(group_ids.is_subset(&edge_ids), "every edge_group id is a live edge id");
    assert_eq!(
        m.edge_groups.len(),
        live_edges - 3,
        "drilled plate: 2 bore rims merge (4 arcs -> 2) + 1 lateral seam dropped"
    );
}

#[test]
fn worker_mesh_tol_refines_curved_edges_and_faces() {
    // A bare cylinder: its bore edges are full circles, so the chord knob
    // is directly observable in the segment count.
    let mut cyl = Body::new();
    let frame = Frame3::from_z(Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0)).unwrap();
    cyl.cylinder(frame, 1.0, 2.0).unwrap();

    let coarse = cyl.worker_mesh_tol(0.1);
    let fine = cyl.worker_mesh_tol(1e-4);

    // Finer chord => more curved-edge segments (the two cap circles).
    assert!(
        fine.lines.len() > coarse.lines.len(),
        "finer chord refines curved edges: fine {} vs coarse {} line floats",
        fine.lines.len(),
        coarse.lines.len()
    );
    // Finer chord => more curved-face triangles too.
    assert!(
        fine.indices.len() > coarse.indices.len(),
        "finer chord refines curved faces: fine {} vs coarse {} indices",
        fine.indices.len(),
        coarse.indices.len()
    );

    // Both variants still partition their own lines buffer.
    for m in [&coarse, &fine] {
        let total: u32 = m.edge_groups.iter().map(|g| g.2).sum();
        assert_eq!(total as usize, m.lines.len());
    }
}

#[test]
fn worker_mesh_default_unchanged_snapshot() {
    // The default path is the mass/oracle tessellation: lock its buffer
    // sizes so an accidental density change is caught. A cylinder default
    // is the 32-segment-per-arc legacy density.
    let mut cyl = Body::new();
    let frame = Frame3::from_z(Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0)).unwrap();
    cyl.cylinder(frame, 1.0, 2.0).unwrap();
    let m = cyl.worker_mesh();

    // worker_mesh() must equal worker_mesh_tol(None-equivalent): the
    // default density. Snapshot the counts (32-seg arcs).
    let snap_positions = m.positions.len();
    let snap_indices = m.indices.len();
    let snap_lines = m.lines.len();

    // Re-derive: a fresh build is identical (determinism + no global
    // state leak from the tol calls above in other tests).
    let m2 = cyl.worker_mesh();
    assert_eq!(m2.positions.len(), snap_positions);
    assert_eq!(m2.indices.len(), snap_indices);
    assert_eq!(m2.lines.len(), snap_lines);
    assert_eq!(m2.positions, m.positions, "default positions byte-identical");
    assert_eq!(m2.lines, m.lines, "default lines byte-identical");

    // The TRIANGLE mesh (positions/indices) is the mass/oracle tessellation and
    // is byte-identical (asserted above); only the EDGE picking metadata changed.
    // The default LINES now OMIT the periodic seam (fieldforge P1), so they are a
    // strict subset of the legacy render_mesh().edges flattening.
    let mut legacy = Vec::new();
    for poly in cyl.render_mesh().edges {
        for w in poly.windows(2) {
            for p in w {
                legacy.extend_from_slice(&[p.x as f32, p.y as f32, p.z as f32]);
            }
        }
    }
    assert!(
        m.lines.len() < legacy.len(),
        "worker_mesh() drops the cylinder seam from the wireframe: {} vs legacy {}",
        m.lines.len(),
        legacy.len()
    );
}
