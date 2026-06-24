//! The worker-protocol mesh surface (swap task 28): one call turns a
//! Body into the exact buffer shape a Three.js-class viewport consumes
//! over a worker boundary (fieldforge's `mesh()` + `meshEdges()` +
//! `faceGroups` contract):
//!
//! - `positions`/`normals`: flat f32 triples, vertices duplicated per
//!   triangle (flat shading; hard-surface CAD reads crisp, and the
//!   crisp look comes from edge lines, not vertex normals).
//! - `indices`: u32 triangle indices (sequential given the duplication,
//!   kept explicit because the consumer indexes).
//! - `face_groups`: per face, the (id, first index, index count) range
//!   into `indices`: the picking layer's raycast table, which the
//!   parity-era RenderMesh (interrogate.rs) does not carry. `id` is the
//!   face's arena slot index (unique among LIVE faces in one snapshot).
//! - `lines`: flat f32 segment PAIRS (LineSegments format), flattened
//!   from RenderMesh's per-edge polylines (exact straight edges,
//!   sampled curves: the proven item-95 machinery).
//! - `edge_groups`: per topological edge, the (id, start, count) range
//!   into `lines`, where `id` is the edge's STABLE `EntityId.0` (so the
//!   host maps a picked/rendered edge back to its `EdgeKey` via
//!   `Body::lookup`), and `start`/`count` are in **f32 elements** of the
//!   `lines` buffer (so the edge's segments are `lines[start..start+count]`,
//!   `count` always a multiple of 6 = two xyz endpoints per segment).
//!   This is the LineSegments analogue of `face_groups`: it lets the host
//!   build a per-edge picking table over the same `lines` buffer the
//!   existing `mesh_lines_ptr/len` consumers read unchanged.
//!
//! Everything here READS the tessellation; nothing mutates geometry.
//!
//! `worker_mesh()` is the DEFAULT-density path (byte-identical to the
//! pre-edge_groups output for positions/normals/indices/lines, and to the
//! mass/oracle tessellation). `worker_mesh_tol(chord)` is an additive
//! coarse/fine variant that refines curved FACES and curved EDGES to the
//! chord tolerance; it never touches the default path.

use crate::body::Body;
use keel_math::vec::Vec3;

#[derive(Debug, Default, Clone)]
pub struct WorkerMesh {
    pub positions: Vec<f32>,
    pub normals: Vec<f32>,
    pub indices: Vec<u32>,
    /// (face id, first index, index count) per face, covering `indices`.
    /// `id` = face arena slot index (`FaceKey::index`).
    pub face_groups: Vec<(u64, u32, u32)>,
    /// Flat segment pairs: [x0,y0,z0, x1,y1,z1, ...] per segment.
    pub lines: Vec<f32>,
    /// (edge id, start, count) per topological edge, covering `lines`.
    /// `id` = edge stable `EntityId.0` (resolve with `Body::lookup`);
    /// `start`/`count` are f32-element offsets into `lines`, so the edge's
    /// segments are `lines[start as usize..(start + count) as usize]` and
    /// `count` is a multiple of 6. Ranges are contiguous, non-overlapping,
    /// and tile the whole `lines` buffer in build order.
    pub edge_groups: Vec<(u64, u32, u32)>,
}

impl Body {
    /// Worker-protocol mesh at the DEFAULT tessellation density. Output is
    /// byte-identical to the mass/oracle tessellation (positions, normals,
    /// indices, lines): this is the path the mass==mesh gate reads, so it
    /// must never change density. `edge_groups`/`face_groups` are additive
    /// metadata over the same buffers.
    pub fn worker_mesh(&self) -> WorkerMesh {
        self.worker_mesh_opt(None)
    }

    /// Worker-protocol mesh at chord tolerance `chord`: curved analytic
    /// FACES are faceted so each triangle stays within `chord` of the true
    /// surface (the item-98 adaptive machinery), and curved EDGES are
    /// arc-sampled to the same chord (finer `chord` => more segments).
    /// Straight faces/edges are unchanged. This is the coarse-draft /
    /// fine-final variant; it is a NEW method and does not affect
    /// `worker_mesh()` (the default/mass path).
    pub fn worker_mesh_tol(&self, chord: f64) -> WorkerMesh {
        self.worker_mesh_opt(Some(chord))
    }

    fn worker_mesh_opt(&self, tol: Option<f64>) -> WorkerMesh {
        let mut out = WorkerMesh::default();
        // Face-grouped triangles (the picking table needs per-face
        // ranges, so this tessellates per face rather than reusing the
        // ungrouped facet list). tol=None is the default density, the
        // exact tessellation the volume oracle uses.
        for face in self.face_keys() {
            let tris = match tol {
                Some(t) => self.tessellate_face_tol(face, t),
                None => self.tessellate_face(face),
            };
            if tris.is_empty() {
                continue;
            }
            let start = out.indices.len() as u32;
            for t in &tris {
                let n = (t[1] - t[0]).cross(t[2] - t[0]);
                let n = n.try_normalize().unwrap_or(Vec3::new(0.0, 0.0, 1.0));
                for p in t {
                    let idx = (out.positions.len() / 3) as u32;
                    out.positions
                        .extend_from_slice(&[p.x as f32, p.y as f32, p.z as f32]);
                    out.normals
                        .extend_from_slice(&[n.x as f32, n.y as f32, n.z as f32]);
                    out.indices.push(idx);
                }
            }
            let count = out.indices.len() as u32 - start;
            // The arena index is unique among LIVE faces, which is all a
            // single mesh snapshot's picking table needs.
            out.face_groups.push((face.index() as u64, start, count));
        }
        // Edge lines: sample every topological edge once (exact for
        // straight, adaptive for curved) and tag each edge's segment range
        // so the host can pick edges. Iterates `self.edges` in the SAME
        // order render_mesh_opt does, and tol=None routes through the same
        // 32-segment default arc sampling, so `lines` is byte-identical to
        // the pre-edge_groups `render_mesh().edges` flattening.
        // Two consumer cleanups (the fieldforge cylinder-seam / bisected-rim P1).
        // These touch ONLY the edge picking metadata (`lines` + `edge_groups`),
        // never the triangle mesh the mass==mesh oracle reads:
        //   (1) SKIP a periodic-face SEAM edge -- a manifold (2-coedge) edge whose
        //       BOTH coedges lie on the SAME face. It is a parametric artifact of
        //       the seamed cylinder/cone/sphere/torus, not a model edge, so it must
        //       not render as a line or be pickable.
        //   (2) MERGE co-circular arcs into ONE group -- a bored rim is split by the
        //       seam into two arcs that share one circle curve; emit them as a single
        //       edge_group so a rim pick highlights/selects the WHOLE circle
        //       (OCCT/Parasolid behavior), not a half.
        #[derive(PartialEq, Eq, Hash, Clone, Copy)]
        enum EKey {
            // co-circular arcs key by the circle's GEOMETRY (center, normal,
            // radius) so the two arcs of one rim merge even across distinct curve
            // instances; non-circle edges key by their own edge (no merge).
            Circle([i64; 7]),
            Solo(crate::entity::EdgeKey),
        }
        let mut buckets: Vec<(u64, Vec<Vec<Vec3>>)> = Vec::new();
        let mut bindex: std::collections::HashMap<EKey, usize> = std::collections::HashMap::new();
        for (ek, _) in self.edges.iter() {
            if self.fins_of_edge(ek).len() == 2 && self.faces_around_edge(ek).len() == 1 {
                continue; // seam: 2 coedges, 1 face
            }
            let Some(poly) = self.edge_polyline_opt(ek, tol) else {
                continue;
            };
            if poly.len() < 2 {
                continue;
            }
            // Stable EntityId so the host maps a picked edge back to its EdgeKey
            // via Body::lookup (the representative arc, for a merged circle).
            let id = self.edge_id(ek).map(|e| e.0).unwrap_or(ek.index() as u64);
            let key = match self
                .edges
                .get(ek)
                .and_then(|e| e.curve)
                .and_then(|(ck, _)| self.curves.get(ck))
            {
                Some(keel_geom::curve::Curve3::Circle(c)) => {
                    let q = |x: f64| (x * 1e6).round() as i64;
                    let n = c.x_axis.cross(c.y_axis);
                    EKey::Circle([
                        q(c.center.x), q(c.center.y), q(c.center.z),
                        q(n.x), q(n.y), q(n.z), q(c.radius),
                    ])
                }
                _ => EKey::Solo(ek),
            };
            let b = *bindex.entry(key).or_insert_with(|| {
                buckets.push((id, Vec::new()));
                buckets.len() - 1
            });
            buckets[b].1.push(poly);
        }
        for (id, polys) in buckets {
            let start = out.lines.len() as u32;
            for poly in &polys {
                // A polyline of n points => n-1 segments => (n-1)*6 floats.
                for w in poly.windows(2) {
                    for p in w {
                        out.lines
                            .extend_from_slice(&[p.x as f32, p.y as f32, p.z as f32]);
                    }
                }
            }
            let count = out.lines.len() as u32 - start;
            if count > 0 {
                out.edge_groups.push((id, start, count));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use keel_geom::surface::Frame3;

    #[test]
    fn worker_mesh_block_shape() {
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.0, 1.0, 1.0).unwrap();
        let m = b.worker_mesh();
        // 6 faces x 2 triangles, duplicated vertices.
        assert_eq!(m.indices.len(), 36, "12 triangles");
        assert_eq!(m.positions.len(), 36 * 3);
        assert_eq!(m.normals.len(), m.positions.len());
        assert_eq!(m.face_groups.len(), 6);
        // Groups tile the index buffer exactly.
        let total: u32 = m.face_groups.iter().map(|g| g.2).sum();
        assert_eq!(total as usize, m.indices.len());
        // 12 straight edges -> 12 segments -> 12 * 2 * 3 floats.
        assert_eq!(m.lines.len(), 12 * 6);
        // 12 edge groups, each one straight segment = 6 floats, tiling lines.
        assert_eq!(m.edge_groups.len(), 12);
        let etotal: u32 = m.edge_groups.iter().map(|g| g.2).sum();
        assert_eq!(etotal as usize, m.lines.len());
        for g in &m.edge_groups {
            assert_eq!(g.2, 6, "each straight edge is one segment");
        }
    }

    #[test]
    fn worker_mesh_drilled_plate() {
        use crate::boolean::{BoolOp, boolean};
        let mut plate = Body::new();
        plate.block(Vec3::ZERO, 4.0, 4.0, 1.0).unwrap();
        let dframe = Frame3::from_z(Vec3::new(2.0, 2.0, -0.5), Vec3::new(0.0, 0.0, 1.0)).unwrap();
        let mut drill = Body::new();
        drill.cylinder(dframe, 1.0, 2.0).unwrap();
        let holed = boolean(&plate, &drill, BoolOp::Difference, 1e-7)
            .unwrap()
            .body;
        let m = holed.worker_mesh();
        assert!(!m.positions.is_empty() && !m.lines.is_empty());
        assert_eq!(m.normals.len(), m.positions.len());
        let total: u32 = m.face_groups.iter().map(|g| g.2).sum();
        assert_eq!(total as usize, m.indices.len());
        // The grouped mesh's signed volume must match mesh_volume (same
        // tessellation through a different packaging).
        let mut v = 0.0f64;
        for tri in m.indices.chunks(3) {
            let p = |i: u32| {
                let k = i as usize * 3;
                Vec3::new(
                    m.positions[k] as f64,
                    m.positions[k + 1] as f64,
                    m.positions[k + 2] as f64,
                )
            };
            let (a, b2, c) = (p(tri[0]), p(tri[1]), p(tri[2]));
            v += a.dot(b2.cross(c)) / 6.0;
        }
        let mv = holed.mesh_volume();
        assert!(
            (v - mv).abs() < 1e-3 * (1.0 + mv.abs()),
            "worker mesh volume {v} vs mesh_volume {mv} (f32 quantization band)"
        );
        // edge_groups tile the lines buffer.
        let etotal: u32 = m.edge_groups.iter().map(|g| g.2).sum();
        assert_eq!(etotal as usize, m.lines.len());
    }
}
