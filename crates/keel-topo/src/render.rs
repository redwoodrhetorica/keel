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
//!   parity-era RenderMesh (interrogate.rs) does not carry.
//! - `lines`: flat f32 segment PAIRS (LineSegments format), flattened
//!   from RenderMesh's per-edge polylines (exact straight edges,
//!   sampled curves: the proven item-95 machinery).
//!
//! Everything here READS the tessellation; nothing mutates geometry.

use crate::body::Body;
use keel_math::vec::Vec3;

#[derive(Debug, Default, Clone)]
pub struct WorkerMesh {
    pub positions: Vec<f32>,
    pub normals: Vec<f32>,
    pub indices: Vec<u32>,
    /// (face id, first index, index count) per face, covering `indices`.
    pub face_groups: Vec<(u64, u32, u32)>,
    /// Flat segment pairs: [x0,y0,z0, x1,y1,z1, ...] per segment.
    pub lines: Vec<f32>,
}

impl Body {
    pub fn worker_mesh(&self) -> WorkerMesh {
        let mut out = WorkerMesh::default();
        // Face-grouped triangles (the picking table needs per-face
        // ranges, so this tessellates per face rather than reusing the
        // ungrouped facet list).
        for face in self.face_keys() {
            let tris = self.tessellate_face(face);
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
        // Edge lines: the parity-era render_mesh already samples every
        // topological edge once (exact for straight, adaptive for
        // curved); flatten its polylines into LineSegments pairs.
        for poly in self.render_mesh().edges {
            for w in poly.windows(2) {
                for p in w {
                    out.lines
                        .extend_from_slice(&[p.x as f32, p.y as f32, p.z as f32]);
                }
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
    }
}
