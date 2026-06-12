//! Ring-face tessellation: the drilled plate's annuli must triangulate
//! as TRUE polygons-with-holes (no triangles covering the bore: the
//! signed-cancellation fan rendered capped holes in every viewer).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use keel_geom::surface::Frame3;
use keel_math::vec::Vec3;
use keel_topo::body::Body;
use keel_topo::boolean::{BoolOp, boolean};

#[test]
fn annulus_triangulation_leaves_the_bore_open() {
    let mut plate = Body::new();
    plate.block(Vec3::ZERO, 4.0, 4.0, 1.0).unwrap();
    let f = Frame3::from_z(Vec3::new(2.0, 2.0, -0.5), Vec3::new(0.0, 0.0, 1.0)).unwrap();
    let mut tool = Body::new();
    tool.cylinder(f, 1.0, 2.0).unwrap();
    let holed = boolean(&plate, &tool, BoolOp::Difference, 1e-7)
        .unwrap()
        .body;
    let mesh = holed.worker_mesh();
    let pos: Vec<[f32; 3]> = mesh
        .positions
        .chunks(3)
        .map(|c| [c[0], c[1], c[2]])
        .collect();
    let mut covering = 0usize;
    for t in mesh.indices.chunks(3) {
        let c = [0, 1, 2].map(|k| pos[t[k] as usize]);
        let cx = (c[0][0] + c[1][0] + c[2][0]) / 3.0 - 2.0;
        let cy = (c[0][1] + c[1][1] + c[2][1]) / 3.0 - 2.0;
        let cz = (c[0][2] + c[1][2] + c[2][2]) / 3.0;
        // Triangle centroid strictly inside the bore (r < 0.9) on the
        // top or bottom face plane.
        if (cx * cx + cy * cy).sqrt() < 0.9 && (cz.abs() < 1e-6 || (cz - 1.0).abs() < 1e-6) {
            covering += 1;
        }
    }
    println!(
        "triangles {} covering-the-hole {}",
        mesh.indices.len() / 3,
        covering
    );
    assert_eq!(covering, 0, "annulus triangles must not cover the bore");
    // The true triangulation must keep the chordal volume agreement.
    let v = holed.mass_properties().unwrap().volume;
    let m = holed.mesh_volume();
    assert!(
        (m - v).abs() <= 2e-2 * (1.0 + v.abs()),
        "mesh {m} vs mass {v}"
    );
}
