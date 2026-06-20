//! Public edge-pick API: the additive accessors that let a CONSUMER
//! enumerate and pick the edges of an arbitrary body (a boolean result),
//! the prerequisite for fillet/chamfer-by-pick in the fieldforge
//! integration. Read-only accessors -- they cannot affect WRONG; these
//! tests prove the boolean -> enumerate -> pick -> fillet flow and the
//! correctness of `closest_point_on_edge` on a straight AND a curved edge.
#![allow(clippy::unwrap_used)]

use keel_geom::curve::Curve3;
use keel_geom::surface::Frame3;
use keel_math::vec::Vec3;
use keel_topo::Body;
use keel_topo::boolean::{BoolOp, boolean};
use keel_topo::entity::EdgeKey;

fn blk(o: Vec3, dx: f64, dy: f64, dz: f64) -> Body {
    let mut b = Body::new();
    b.block(o, dx, dy, dz).unwrap();
    b
}

fn cyl(pos: Vec3, axis: Vec3, r: f64, h: f64) -> Body {
    let mut b = Body::new();
    b.cylinder(Frame3::from_z(pos, axis).unwrap(), r, h).unwrap();
    b
}

fn endpoints(b: &Body, e: EdgeKey) -> (Vec3, Vec3) {
    let edge = b.edge(e).unwrap();
    let (va, vb) = edge.bounds;
    (b.vertex(va).unwrap().point, b.vertex(vb).unwrap().point)
}

/// Analytic mass equals the tessellated mesh volume (the
/// DECLINE-never-WRONG self-consistency the floor gate guards).
fn assert_mass_eq_mesh(b: &Body, ctx: &str) {
    assert!(b.validate().is_ok(), "{ctx}: invalid body");
    let mass = b
        .mass_properties()
        .unwrap_or_else(|e| panic!("{ctx}: mass declined: {e:?}"))
        .volume;
    let mesh = b.mesh_volume();
    assert!(mass > 0.0, "{ctx}: non-positive mass {mass}");
    assert!(
        (mass - mesh).abs() < 2e-2 * (1.0 + mass),
        "{ctx}: analytic mass {mass} != mesh {mesh} (gap {:.3}%)",
        100.0 * (mass - mesh).abs() / mass.max(1e-9)
    );
}

/// The fieldforge scenario: union two overlapping blocks, then ENUMERATE
/// the result's edges, PICK a seam edge by point, and FILLET the picked
/// edge to a watertight result. This is exactly what fillet-by-pick needs
/// and could not do before these accessors were public.
#[test]
fn boolean_union_enumerate_pick_fillet() {
    // Base: 20 x 20 x 10, z in [0,10]. Tower: 10 x 10 x 12, z in [8,20],
    // x,y in [-5,5] -- it OVERLAPS the base in z in [8,10], so the union
    // is one watertight stepped solid (no coplanar-face tangency).
    let base = blk(Vec3::new(-10.0, -10.0, 0.0), 20.0, 20.0, 10.0);
    let tower = blk(Vec3::new(-5.0, -5.0, 8.0), 10.0, 10.0, 12.0);
    let body = boolean(&base, &tower, BoolOp::Union, 1e-7)
        .expect("union declined")
        .body;
    assert_mass_eq_mesh(&body, "union stepped solid");

    // (1) Enumeration: the union result exposes its edges -- the whole
    // point. A freshly built primitive used to be the only enumerable
    // body; a boolean result now is too.
    let edges = body.edge_keys();
    assert!(
        edges.len() >= 12,
        "stepped solid should have many edges, got {}",
        edges.len()
    );
    // edge_keys must be a subset of the body's live edges and resolvable.
    assert!(edges.iter().all(|&e| body.edge(e).is_some()));

    // (2) Pick a SEAM edge: the step ring at z = 10 where the tower meets
    // the base top is created by the union. Point just above the step's
    // mid-front (0, 5, 10).
    let seam_pt = Vec3::new(0.0, 5.0, 10.0);
    let seam_edge = body.nearest_edge(seam_pt).expect("no nearest edge");
    let (sa, sb) = endpoints(&body, seam_edge);
    let (_, seam_d) = body.closest_point_on_edge(seam_edge, seam_pt).unwrap();
    assert!(
        seam_d < 1e-6,
        "picked seam edge is not on the seam point (dist {seam_d})"
    );
    assert!(
        (sa.z - 10.0).abs() < 1e-6 && (sb.z - 10.0).abs() < 1e-6,
        "picked seam edge is not on the z=10 step: {sa:?}..{sb:?}"
    );

    // (3) Pick a CONVEX top edge of the result and FILLET it: the tower's
    // top-front edge runs (-5,5,20)..(5,5,20) -- a simple convex
    // planar-planar edge produced by the union, away from the stepped
    // vertices. Pick it by its midpoint.
    let top_pt = Vec3::new(0.0, 5.0, 20.0);
    let top_edge = body.nearest_edge(top_pt).expect("no top edge");
    let (ta, tb) = endpoints(&body, top_edge);
    let (_, top_d) = body.closest_point_on_edge(top_edge, top_pt).unwrap();
    assert!(
        top_d < 1e-6,
        "picked top edge not on the pick point (dist {top_d})"
    );
    assert!(
        (ta.z - 20.0).abs() < 1e-6 && (tb.z - 20.0).abs() < 1e-6,
        "picked top edge is not on the z=20 tower top: {ta:?}..{tb:?}"
    );

    // Fillet the picked edge: enumerate -> pick -> fillet must yield a
    // valid, watertight body whose analytic mass equals its mesh.
    let filleted = body
        .fillet_edge(top_edge, 1.5)
        .expect("fillet of picked union edge declined");
    assert_mass_eq_mesh(&filleted, "filleted picked union edge");
}

/// `closest_point_on_edge` on a known STRAIGHT edge: exact segment
/// projection, including the clamp past an endpoint.
#[test]
fn closest_point_on_straight_edge() {
    let b = blk(Vec3::ZERO, 2.0, 2.0, 2.0);
    // The bottom edge from (0,0,0) to (2,0,0).
    let e = b
        .edge_keys()
        .into_iter()
        .find(|&e| {
            let (a, c) = endpoints(&b, e);
            let mut pts = [a, c];
            pts.sort_by(|u, v| u.x.total_cmp(&v.x));
            (pts[0] - Vec3::new(0.0, 0.0, 0.0)).norm() < 1e-9
                && (pts[1] - Vec3::new(2.0, 0.0, 0.0)).norm() < 1e-9
        })
        .expect("no (0,0,0)-(2,0,0) edge");

    // Interior projection: p above the segment midpoint.
    let (q, d) = b.closest_point_on_edge(e, Vec3::new(1.0, -1.0, 0.0)).unwrap();
    assert!((q - Vec3::new(1.0, 0.0, 0.0)).norm() < 1e-9, "q = {q:?}");
    assert!((d - 1.0).abs() < 1e-9, "d = {d}");

    // Past the +x end: clamps to the endpoint (2,0,0).
    let (q2, d2) = b.closest_point_on_edge(e, Vec3::new(3.0, 0.0, 0.0)).unwrap();
    assert!((q2 - Vec3::new(2.0, 0.0, 0.0)).norm() < 1e-9, "q2 = {q2:?}");
    assert!((d2 - 1.0).abs() < 1e-9, "d2 = {d2}");
}

/// `closest_point_on_edge` on a CURVED edge: a cylinder's top cap circle
/// (radius 5, center (0,0,10)). The closed-form conic projection must be
/// exact for points on the axis, in-plane, and off-axis.
#[test]
fn closest_point_on_curved_edge() {
    let r = 5.0;
    let c = cyl(Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0), r, 10.0);
    // Find a circular edge near the top cap (center z = 10).
    let circ = c
        .edge_keys()
        .into_iter()
        .find(|&e| {
            let Some((ck, _)) = c.edge(e).and_then(|x| x.curve) else {
                return false;
            };
            matches!(c.curve(ck), Some(Curve3::Circle(cc)) if (cc.center.z - 10.0).abs() < 1e-6)
        })
        .expect("no top circular edge on the cylinder");

    // In-plane external point along +x: closest point is (5,0,10), d = 5.
    let (q, d) = c.closest_point_on_edge(circ, Vec3::new(10.0, 0.0, 10.0)).unwrap();
    assert!((d - 5.0).abs() < 1e-7, "in-plane d = {d}");
    assert!((q - Vec3::new(5.0, 0.0, 10.0)).norm() < 1e-7, "in-plane q = {q:?}");
    // The returned point lies on the circle (radius r about (0,0,10)).
    assert!(
        ((q - Vec3::new(0.0, 0.0, 10.0)).norm() - r).abs() < 1e-7,
        "q not on the circle: {q:?}"
    );

    // On the axis above the cap: every circle point is sqrt(r^2 + 100)
    // away. The closed form must NOT collapse to a chord/endpoint.
    let (_, d_axis) = c.closest_point_on_edge(circ, Vec3::new(0.0, 0.0, 20.0)).unwrap();
    let want = (r * r + 100.0).sqrt();
    assert!((d_axis - want).abs() < 1e-7, "on-axis d = {d_axis}, want {want}");

    // A point exactly on the circle reports ~zero distance (the curve is
    // sampled, not chorded, so a true on-curve point is recovered).
    let on = Vec3::new(0.0, r, 10.0);
    let (_, d_on) = c.closest_point_on_edge(circ, on).unwrap();
    assert!(d_on < 1e-7, "on-curve d = {d_on}");
}
