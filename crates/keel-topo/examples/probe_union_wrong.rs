//! Probe the shell->mirror->fillet->union(cyl) WRONG (seed 11400715918834827198):
//! dissect the malformed union result so the gate hole is precisely localized.
//!
//! Run: cargo run --release -p keel-topo --example probe_union_wrong
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
use keel_geom::surface::Frame3;
use keel_math::vec::Vec3;
use keel_topo::Body;
use keel_topo::boolean::{BoolOp, boolean};
use keel_topo::entity::{AnyKey, EdgeKey};

fn edge_keys(b: &Body) -> Vec<EdgeKey> {
    b.entity_ids()
        .filter_map(|id| match b.lookup(id) {
            Some(AnyKey::Edge(k)) => Some(k),
            _ => None,
        })
        .collect()
}
fn edge_endpoints(b: &Body, e: EdgeKey) -> Option<(Vec3, Vec3)> {
    let edge = b.edge(e)?;
    let (va, vb) = edge.bounds;
    Some((b.vertex(va)?.point, b.vertex(vb)?.point))
}
fn find_edge(b: &Body, p: Vec3, q: Vec3, tol: f64) -> Option<EdgeKey> {
    let close = |a: Vec3, c: Vec3| (a - c).norm() <= tol;
    edge_keys(b).into_iter().find(|&e| {
        edge_endpoints(b, e)
            .map(|(a, c)| (close(a, p) && close(c, q)) || (close(a, q) && close(c, p)))
            .unwrap_or(false)
    })
}

fn main() {
    let (dx, dy, dz) = (16.615817370044468, 25.16818664099246, 24.23722896202797);
    let mut block = Body::new();
    block
        .block(Vec3::new(-dx * 0.5, -dy * 0.5, -dz * 0.5), dx, dy, dz)
        .unwrap();
    let shelled = block.hollow(1.0523181503240229).unwrap();
    let mirrored = shelled
        .mirrored(
            Vec3::new(0.0, 0.0, 12.118614481013985),
            Vec3::new(0.0, 0.0, 1.0),
        )
        .unwrap();
    let mirror_union = boolean(&shelled, &mirrored, BoolOp::Union, 1e-7)
        .unwrap()
        .body;
    let e = find_edge(
        &mirror_union,
        Vec3::new(-7.255590534698211, -11.531775170172207, 11.066296330689962),
        Vec3::new(7.255590534698211, -11.531775170172207, 11.066296330689962),
        1e-6,
    )
    .unwrap();
    let body = mirror_union.fillet_edge(e, 0.813899886354287).unwrap();

    let base = Vec3::new(
        -1.6013419572753496,
        -2.3088891422783124,
        -10.426150362427773,
    );
    let mut tool = Body::new();
    tool.cylinder(
        Frame3::from_z(base, Vec3::new(0.0, 0.0, 1.0)).unwrap(),
        4.345903311368783,
        79.852113674645,
    )
    .unwrap();

    let report = |tag: &str, b: &Body| {
        let comps = b.connected_components();
        let c = b.counts();
        let mass = b.mass_properties().map(|m| m.volume);
        eprintln!(
            "{tag}: comps={} genus={} regions={} shells={} v={} e={} f={} | mass={:?} mesh_vol={:.4} tess_vol={:.4} validate={:?}",
            comps.len(),
            c.genus,
            c.regions,
            c.shells,
            c.v,
            c.e,
            c.f,
            mass,
            b.mesh_volume(),
            b.tessellated_volume(),
            b.validate().is_ok(),
        );
        // Per-component shell count.
        for (i, comp) in comps.iter().enumerate() {
            eprintln!("    comp[{i}]: {} shell(s)", comp.len());
        }
    };

    report("pre-union body", &body);
    report("tool", &tool);

    match boolean(&body, &tool, BoolOp::Union, 1e-7) {
        Ok(r) => {
            eprintln!("UNION RETURNED Ok (faults={:?})", r.faults);
            report("union result", &r.body);
            // Independent Monte-Carlo truth of the UNION volume = vol(inside
            // body OR inside tool), sampled in the combined AABB. Tells us
            // whether mass (10009) or mesh (5242) is the correct value.
            let mc = mc_union_volume(&body, &tool);
            eprintln!("MC union truth ~= {mc:.1}");
        }
        Err(e) => eprintln!("UNION DECLINED: {e:?}"),
    }
}

/// Monte-Carlo |A u B| via the generalized winding number inside each operand.
fn mc_union_volume(a: &Body, b: &Body) -> f64 {
    let ba = a.bounding_box();
    let bb = b.bounding_box();
    let lo = Vec3::new(
        ba.min.x.min(bb.min.x),
        ba.min.y.min(bb.min.y),
        ba.min.z.min(bb.min.z),
    );
    let hi = Vec3::new(
        ba.max.x.max(bb.max.x),
        ba.max.y.max(bb.max.y),
        ba.max.z.max(bb.max.z),
    );
    let vol_box = (hi.x - lo.x) * (hi.y - lo.y) * (hi.z - lo.z);
    let n = 400_000u64;
    let mut hits = 0u64;
    let mut s = 0x1234_5678_9abc_def0u64;
    let mut rf = || {
        s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (s >> 11) as f64 / (1u64 << 53) as f64
    };
    for _ in 0..n {
        let p = Vec3::new(
            lo.x + rf() * (hi.x - lo.x),
            lo.y + rf() * (hi.y - lo.y),
            lo.z + rf() * (hi.z - lo.z),
        );
        let ina = a.generalized_winding_number(p).abs() > 0.5;
        let inb = b.generalized_winding_number(p).abs() > 0.5;
        if ina || inb {
            hits += 1;
        }
    }
    vol_box * (hits as f64) / (n as f64)
}
