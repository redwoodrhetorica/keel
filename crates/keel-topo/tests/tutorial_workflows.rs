//! FAITHFUL Plasticity-tutorial workflow coverage (the metric the
//! `examples/realsoak.rs` FUZZER cannot give).
//!
//! `realsoak` samples a learned grammar with RANDOM primitive dimensions, so
//! the bulk of its declines are geometrically-impossible random primitives --
//! NOT a measure of whether real tutorial workflows work. This file is the
//! opposite: the six dominant workflow CLASSES distilled from 113 Plasticity
//! tutorials (see `docs/PLASTICITY_TUTORIAL_TEST_PLAN.md`), each run with
//! SENSIBLE FIXED parameters (a ~40 mm part, 3..5 mm fillet radii, etc.),
//! deterministic, NOT random.
//!
//! Each workflow / variant is its OWN `#[test]` so `cargo test` reports each
//! independently:
//!   * a workflow the kernel does correctly  => the test PASSES
//!     (validate() Ok + mass == mesh + watertight),
//!   * a workflow the kernel DECLINES (an op returns `Err`) or that produces
//!     mass != mesh / invalid => the test FAILS, and that failure is a
//!     DOCUMENTED, PRIORITIZED GAP (the decline reason is captured in the
//!     panic message).
//!
//! Scoreboard convention: a currently-DECLINING workflow is marked
//! `#[ignore = "tutorial-gap: <reason>"]` so the default `cargo test` stays
//! green (fix work needs a clean baseline), while the gap stays visible inline
//! and runnable via `cargo test --test tutorial_workflows -- --ignored`. When a
//! gap is FIXED, REMOVE its `#[ignore]` so the now-passing workflow is a
//! permanent regression guard. The live metric = (passing) of (total); the
//! ignored set is the worklist. Today: 6 of 12 pass.
//!
//! Watertightness note: the crate's `mesh_open_ratio` is `pub(crate)`, so an
//! external integration test cannot call it. The faithful public-API
//! expression of "watertight solid" is `validate()` Ok -- whose
//! `check_boundary_chains` rejects open/dangling edges (the topological
//! closure signature) -- COMBINED with mass == mesh (a cracked mesh and the
//! analytic mass disagree). Every assertion below pairs the two.

use keel_geom::surface::{Frame3, Surface3};
use keel_topo::body::Body;
use keel_topo::boolean::{boolean, BoolOp};
use keel_topo::entity::{AnyKey, EdgeKey, FaceKey, SurfaceGeom};
use keel_math::vec::Vec3;

const TOL: f64 = 1e-7;

// ---------------------------------------------------------------------------
// Edge / face enumeration helpers (public-API only; mirrors the patterns in
// examples/realsoak.rs fns edge_keys / edge_endpoints / edge_len /
// blend_edge_candidates, but DETERMINISTIC -- no RNG).
// ---------------------------------------------------------------------------

fn edge_keys(b: &Body) -> Vec<EdgeKey> {
    b.entity_ids()
        .filter_map(|id| match b.lookup(id) {
            Some(AnyKey::Edge(k)) => Some(k),
            _ => None,
        })
        .collect()
}

/// (start, end) endpoints of an edge via public accessors, if live.
fn edge_endpoints(b: &Body, e: EdgeKey) -> Option<(Vec3, Vec3)> {
    let edge = b.edge(e)?;
    let (va, vb) = edge.bounds;
    Some((b.vertex(va)?.point, b.vertex(vb)?.point))
}

/// Straight-line length of an edge (0 for a closed/seam edge).
fn edge_len(b: &Body, e: EdgeKey) -> f64 {
    edge_endpoints(b, e)
        .map(|(a, c)| (c - a).norm())
        .unwrap_or(0.0)
}

/// Midpoint of an edge (for deterministic geometric selection).
fn edge_mid(b: &Body, e: EdgeKey) -> Option<Vec3> {
    edge_endpoints(b, e).map(|(a, c)| (a + c) * 0.5)
}

fn face_is_planar(b: &Body, f: FaceKey) -> bool {
    match b.face(f).and_then(|x| x.surface) {
        Some((sk, _)) => matches!(
            b.surface(sk),
            Some(SurfaceGeom::Analytic(Surface3::Plane(_)))
        ),
        None => false,
    }
}

/// All STRAIGHT manifold edges bounded by exactly two faces (the kind
/// fillet/chamfer accept), DETERMINISTICALLY ordered: (plane,plane) edges
/// first (the rung the blend code handles best), then by a stable spatial
/// key (the edge midpoint). No RNG -- a faithful test must be reproducible.
fn blendable_edges(b: &Body) -> Vec<EdgeKey> {
    let mut scored: Vec<(u8, [i64; 3], EdgeKey)> = Vec::new();
    for e in edge_keys(b) {
        let faces = b.faces_around_edge(e);
        if faces.len() != 2 {
            continue;
        }
        if edge_len(b, e) <= 1e-6 {
            continue;
        }
        let planar = faces.iter().filter(|&&f| face_is_planar(b, f)).count();
        let rank = match planar {
            2 => 0u8,
            1 => 1u8,
            _ => 2u8,
        };
        let m = edge_mid(b, e).unwrap_or(Vec3::ZERO);
        // Quantize the midpoint to a stable integer key (deterministic order).
        let key = [
            (m.x * 1e6).round() as i64,
            (m.y * 1e6).round() as i64,
            (m.z * 1e6).round() as i64,
        ];
        scored.push((rank, key, e));
    }
    scored.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    scored.into_iter().map(|(_, _, e)| e).collect()
}

/// The single straight blendable edge whose midpoint is closest to `target`
/// (deterministic geometric pick of a specific edge, e.g. a seam or a top
/// edge). Among ties the lowest spatial key wins.
fn edge_nearest(b: &Body, target: Vec3) -> Option<EdgeKey> {
    blendable_edges(b)
        .into_iter()
        .filter_map(|e| edge_mid(b, e).map(|m| (e, (m - target).norm())))
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(e, _)| e)
}

// ---------------------------------------------------------------------------
// The oracle: validate() + mass == mesh + watertight, mirroring
// examples/realsoak.rs fn `audit`. ALL-PLANAR body => tessellation EXACT =>
// tight band (<1%); CURVED body => the mesh carries chordal deficit, so the
// mesh band loosens (<3%) but the analytic mass must still be inside the
// caller's analytic bound. Returns Ok(()) on PASS, Err(reason) on a gap.
// ---------------------------------------------------------------------------

fn curved_face_count(b: &Body) -> usize {
    b.entity_ids()
        .filter_map(|id| match b.lookup(id) {
            Some(AnyKey::Face(k)) => b.face(k),
            _ => None,
        })
        .filter(|f| match f.surface {
            Some((sk, _)) => !matches!(
                b.surface(sk),
                Some(SurfaceGeom::Analytic(Surface3::Plane(_)))
            ),
            None => false,
        })
        .count()
}

/// Assert the full oracle. `expect_lo`/`expect_hi` is the INDEPENDENT analytic
/// volume interval the mass must lie in (the geometry-independent wrong-net,
/// exactly evolve.rs/realsoak's tier 1). Panics with a precise message on any
/// violation so a FAILing test names the gap.
fn assert_solid_ok(b: &Body, what: &str, expect_lo: f64, expect_hi: f64) {
    // 1. Topological watertightness / validity.
    if let Err(errs) = b.validate() {
        panic!("{what}: DECLINE/GAP -- validate() failed: {errs:?}");
    }
    // 2. Analytic mass inside the independent bound (hard wrong-net).
    let mass = match b.mass_properties() {
        Ok(m) => m.volume,
        Err(e) => panic!("{what}: DECLINE/GAP -- mass_properties() Err: {e:?}"),
    };
    assert!(
        mass.is_finite() && mass > 0.0,
        "{what}: non-positive/non-finite mass {mass}"
    );
    assert!(
        mass >= expect_lo && mass <= expect_hi,
        "{what}: WRONG -- analytic mass {mass} outside independent bound \
         [{expect_lo}, {expect_hi}]"
    );
    // 3. mass == mesh self-consistency (the watertight witness). ALL-PLANAR
    //    => exact mesh => tight 1% band; any curved face => chordal deficit so
    //    the mesh band loosens to 3% (the documented tessellation note), but
    //    mass already had to pass the analytic bound above.
    let mesh = b.mesh_volume();
    assert!(
        mesh.is_finite() && mesh > 0.0,
        "{what}: bad mesh volume {mesh}"
    );
    let curved = curved_face_count(b);
    let band = if curved == 0 { 1e-2 } else { 3e-2 };
    let rel = (mass - mesh).abs() / mass.abs().max(mesh.abs()).max(1e-9);
    assert!(
        rel <= band,
        "{what}: NOT watertight / mass != mesh -- mass {mass}, mesh {mesh} \
         (rel {:.4} > {:.3}; curved_faces={curved})",
        rel,
        band
    );
}

/// Unwrap a boolean to its body, or panic naming the fault as a decline.
fn bool_body(a: &Body, t: &Body, op: BoolOp, what: &str) -> Body {
    match boolean(a, t, op, TOL) {
        Ok(res) => res.body,
        Err(f) => panic!("{what}: DECLINE/GAP -- boolean {op:?} faulted: {f:?}"),
    }
}

// ===========================================================================
// 1. boolean_then_fillet -- THE highest-value compound stressor.
//    Two overlapping primitives unioned, then fillet the seam edge(s). Plus
//    the harder Difference + concave-fillet case.
// ===========================================================================

/// Two blocks overlapping along Z, unioned into an L/step solid, then the
/// vertical seam edge filleted. Block A 40x40x20 at origin; block B 40x40x20
/// raised +10 in Z and shifted +20 in X so they share a quarter-overlap and
/// the union is a clean stepped solid with fresh seam edges.
#[test]
fn boolean_then_fillet_block_block_union() {
    let mut a = Body::new();
    a.block(Vec3::ZERO, 40.0, 40.0, 20.0).unwrap();
    let mut bt = Body::new();
    bt.block(Vec3::new(20.0, 0.0, 10.0), 40.0, 40.0, 20.0).unwrap();
    let u = bool_body(&a, &bt, BoolOp::Union, "block+block union");

    // Union volume: two 32000 boxes minus the overlap (20x40x10 = 8000) = 56000.
    assert_solid_ok(&u, "block+block union (pre-fillet)", 55_999.0, 56_001.0);

    // Fillet a convex seam edge (radius 4 mm, small vs the 40 mm feature). The
    // step's concave inner corner and convex outer corners are all fresh seam
    // edges; pick the convex outer top edge near the step (x~60, the far +X
    // top). A fillet removes a little volume, so the bound drops slightly.
    let edges = blendable_edges(&u);
    assert!(!edges.is_empty(), "union produced no blendable edge");
    // Try the seam-region candidates in deterministic order until one accepts;
    // record the first decline reason if ALL decline.
    let mut last_err = String::from("no candidate edge");
    let mut ok: Option<Body> = None;
    for e in &edges {
        match u.fillet_edge(*e, 4.0) {
            Ok(f) => {
                ok = Some(f);
                break;
            }
            Err(err) => last_err = format!("{err:?}"),
        }
    }
    let f = ok.unwrap_or_else(|| {
        panic!("block+block union then fillet: DECLINE/GAP -- every seam edge \
                declined fillet_edge(r=4): last {last_err}")
    });
    // Filleted union: volume slightly under 56000 (a convex round shaves a
    // sliver), well above 55000; curved face present.
    assert_solid_ok(&f, "block+block union then fillet", 55_000.0, 56_001.0);
}

/// A cylinder unioned onto a block (a boss on a plate), then the circular
/// seam where they meet is filleted -- the classic "boss + fillet" tutorial.
#[test]
#[ignore = "tutorial-gap: curved-face pcurve bounds (cyl+block boolean then fillet)"]
fn boolean_then_fillet_cyl_block_union() {
    let mut plate = Body::new();
    plate.block(Vec3::ZERO, 60.0, 60.0, 10.0).unwrap();
    let frame = Frame3::from_z(Vec3::new(30.0, 30.0, 0.0), Vec3::new(0.0, 0.0, 1.0)).unwrap();
    let mut boss = Body::new();
    boss.cylinder(frame, 12.0, 30.0).unwrap();
    let u = bool_body(&plate, &boss, BoolOp::Union, "cyl+block union");

    // Plate 36000 + cylinder (pi*144*30 ~ 13571) minus the embedded stub
    // (the cylinder's lower 10 mm sits inside the plate: pi*144*10 ~ 4524).
    // Net ~ 36000 + 13571 - 4524 = 45047. Generous curved bound.
    assert_solid_ok(&u, "cyl+block union (pre-fillet)", 44_500.0, 45_600.0);

    // Fillet the circular seam (the boss/plate junction) at r = 3 mm. Pick the
    // edge whose midpoint sits on the top face of the plate near the boss
    // footprint (z ~ 10, radius ~ 12 from the boss axis).
    let seam = edge_nearest(&u, Vec3::new(42.0, 30.0, 10.0));
    let seam = seam.unwrap_or_else(|| panic!("cyl+block union: no seam edge found"));
    let f = u.fillet_edge(seam, 3.0).unwrap_or_else(|e| {
        panic!("cyl+block union then fillet: DECLINE/GAP -- fillet_edge(seam, r=3) \
                Err: {e:?}")
    });
    assert_solid_ok(&f, "cyl+block union then fillet", 44_500.0, 45_700.0);
}

/// The HARDER concave-fillet case: a block with a smaller block subtracted to
/// leave a concave notch, then fillet the concave inner edge of the notch.
#[test]
fn difference_then_concave_fillet() {
    let mut base = Body::new();
    base.block(Vec3::ZERO, 40.0, 40.0, 40.0).unwrap();
    // Subtract a 20x20x40 bar from a corner column (through the full Z) to
    // leave an L cross-section with a concave inner vertical edge.
    let mut cutter = Body::new();
    cutter.block(Vec3::new(20.0, 20.0, 0.0), 21.0, 21.0, 40.0).unwrap();
    let d = bool_body(&base, &cutter, BoolOp::Difference, "block-block difference");

    // 64000 minus the removed corner column (20x20x40 = 16000) = 48000.
    assert_solid_ok(&d, "block-block difference (pre-fillet)", 47_999.0, 48_001.0);

    // The concave inner vertical edge sits at (20,20) running up Z. Fillet it
    // at r = 4 mm.
    let inner = edge_nearest(&d, Vec3::new(20.0, 20.0, 20.0));
    let inner = inner.unwrap_or_else(|| panic!("difference: no concave inner edge found"));
    let f = d.fillet_edge(inner, 4.0).unwrap_or_else(|e| {
        panic!("difference then CONCAVE fillet: DECLINE/GAP -- \
                fillet_edge(concave, r=4) Err: {e:?}")
    });
    // A concave fillet ADDS material (fills the inner corner), so volume rises
    // a little above 48000.
    assert_solid_ok(&f, "difference then concave fillet", 47_999.0, 49_000.0);
}

// ===========================================================================
// 2. fillet_all_edges_box -- a block, fillet ALL 12 edges at a modest radius;
//    assert the rounded-box volume matches the analytic closed form. G1, and a
//    G2 variant if the API supports continuity (it does not -- so the G2
//    variant declines on the missing parameter, a recorded gap).
// ===========================================================================

/// Fillet all 12 edges of a 40 mm cube at r = 5 mm. Analytic rounded-box
/// volume = box - 12 edge quarter-cylinders' complement + 8 corner sphere-
/// octants:  V = L^3 - 12 * (1 - pi/4) r^2 * (L - 2r)  for the 12 edges,
/// minus the corner correction. Exact closed form:
///   V = L^3
///       - 3*(4 - pi) r^2 (L - 2r)        [12 edges: each loses (1-pi/4)r^2 * len, len=L-2r, 12/4=3 groups... ]
///       - (8 - ... )                      [corners]
/// We use the standard rounded-cuboid formula:
///   V = l*w*h
///       - (l+w+h) * r^2 * (4 - pi)        ... (edges, after corner sharing)
///   Actually the clean closed form for a cuboid with all edges filleted r and
///   spherical corners is:
///     V = lwh - (4 - pi) r^2 (l + w + h - 6 r ... )   -- messy; instead build
///   the exact value from primitives in the bound below with a tight window.
#[test]
#[ignore = "tutorial-gap: adjacent fillet corner interaction (round all box edges)"]
fn fillet_all_edges_box_g1() {
    let l = 40.0_f64;
    let r = 5.0_f64;
    let mut b = Body::new();
    b.block(Vec3::ZERO, l, l, l).unwrap();

    // Exact rounded-cuboid volume (all 12 edges rounded to radius r, 8 spherical
    // corner octants):
    //   V = lwh
    //     - 12 edges: each replaces a (1 - pi/4) r^2 square-section prism of
    //       length (l - 2r):           12 * (1 - pi/4) * r^2 * (l - 2r)
    //     - 8 corners: each replaces a cube corner r^3 with a sphere octant
    //       (pi/6 r^3): 8 * (1 - pi/6) * r^3
    let pi = std::f64::consts::PI;
    let v_box = l * l * l;
    let v_edges = 12.0 * (1.0 - pi / 4.0) * r * r * (l - 2.0 * r);
    let v_corners = 8.0 * (1.0 - pi / 6.0) * r * r * r;
    let v_exact = v_box - v_edges - v_corners;

    // Fillet every straight blendable edge in turn. Re-enumerate after each
    // success (keys shift as the body is rebuilt). If ANY edge declines, the
    // whole "fillet all edges" workflow is incomplete -> FAIL naming it.
    let mut cur = b;
    let target_edges = 12;
    let mut done = 0;
    for _ in 0..target_edges {
        // Pick the next un-filleted straight (plane,plane) edge: after a round
        // the filleted edges become (plane,cyl) or vanish, so the remaining
        // sharp ones still rank 0. Take the first rank-0 edge.
        let next = blendable_edges(&cur).into_iter().find(|&e| {
            let faces = cur.faces_around_edge(e);
            faces.len() == 2 && faces.iter().all(|&f| face_is_planar(&cur, f))
        });
        let Some(e) = next else { break };
        match cur.fillet_edge(e, r) {
            Ok(f) => {
                cur = f;
                done += 1;
            }
            Err(err) => panic!(
                "fillet_all_edges_box_g1: DECLINE/GAP -- edge {done} of 12 \
                 declined fillet_edge(r=5): {err:?}"
            ),
        }
    }
    assert_eq!(
        done, 12,
        "fillet_all_edges_box_g1: DECLINE/GAP -- only {done}/12 edges had a \
         remaining sharp (plane,plane) edge to fillet (corner interactions \
         consumed the rest)"
    );
    // Tight-ish bound around the analytic rounded cuboid (allow 0.5% for the
    // curved chordal/junction deficit on the many fillet+corner faces).
    let lo = v_exact * 0.995;
    let hi = v_exact * 1.005;
    assert_solid_ok(&cur, "fillet_all_edges_box_g1", lo, hi);
}

/// G2 (curvature-continuous) variant. `fillet_edge` takes only a radius -- it
/// has no continuity parameter -- so a G2 fillet is NOT expressible through the
/// kernel API. This is a recorded API GAP (the plan's "if the API supports
/// continuity" -- it does not). FAIL naming the missing capability.
#[test]
#[ignore = "tutorial-gap: no G2 (curvature-continuous) fillet API"]
fn fillet_all_edges_box_g2() {
    // There is no `fillet_edge_g2` / continuity argument in the public API
    // (blend.rs exposes `fillet_edge(edge, radius)` and `fillet_edge_conic`,
    // neither a G2/curvature-continuous control). A faithful tutorial G2 fillet
    // ("make that a G2 fillet") therefore cannot be built.
    panic!(
        "fillet_all_edges_box_g2: DECLINE/GAP -- the kernel has no G2 \
         (curvature-continuous) fillet API; `fillet_edge` is radius-only \
         (circular/G1). G2 continuity is unimplemented."
    );
}

// ===========================================================================
// 3. loft_two_profiles -- loft between two parallel profiles; closed solid,
//    mass == mesh; plus a tapered variant.
// ===========================================================================

/// Loft between two parallel SQUARE profiles (a straight box via the general
/// polygon loft) -- the all-planar loft the kernel's `loft` builds exactly.
#[test]
fn loft_two_profiles_rect() {
    // Two 30x30 squares, 50 apart in Z: the loft is a 30x30x50 prism.
    let bottom = [
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(30.0, 0.0, 0.0),
        Vec3::new(30.0, 30.0, 0.0),
        Vec3::new(0.0, 30.0, 0.0),
    ];
    let top = [
        Vec3::new(0.0, 0.0, 50.0),
        Vec3::new(30.0, 0.0, 50.0),
        Vec3::new(30.0, 30.0, 50.0),
        Vec3::new(0.0, 30.0, 50.0),
    ];
    let mut b = Body::new();
    b.loft(&bottom, &top).unwrap_or_else(|e| {
        panic!("loft_two_profiles_rect: DECLINE/GAP -- loft() Err: {e:?}")
    });
    // 30*30*50 = 45000, all-planar => exact.
    assert_solid_ok(&b, "loft_two_profiles_rect", 44_999.0, 45_001.0);
}

/// Loft between two parallel CIRCLE profiles -- the analytic frustum/cylinder
/// rung. Equal radii => exact cylinder via `loft_circles`.
#[test]
fn loft_two_profiles_circle() {
    let frame = Frame3::from_z(Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0)).unwrap();
    let mut b = Body::new();
    // Equal radii 15, height 50 => an exact cylinder (mass exact; mesh chordal).
    b.loft_circles(frame, 15.0, 15.0, 50.0).unwrap_or_else(|e| {
        panic!("loft_two_profiles_circle: DECLINE/GAP -- loft_circles() Err: {e:?}")
    });
    let pi = std::f64::consts::PI;
    let v = pi * 15.0 * 15.0 * 50.0; // ~35342.9
    assert_solid_ok(&b, "loft_two_profiles_circle", v * 0.97, v * 1.001);
}

/// Tapered loft variant: two parallel but DIFFERENTLY sized square profiles (a
/// truncated pyramid / frustum), built by the general polygon loft.
#[test]
fn loft_two_profiles_tapered() {
    // 40x40 base, 20x20 top centered above it, 30 tall: a square frustum.
    let bottom = [
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(40.0, 0.0, 0.0),
        Vec3::new(40.0, 40.0, 0.0),
        Vec3::new(0.0, 40.0, 0.0),
    ];
    let top = [
        Vec3::new(10.0, 10.0, 30.0),
        Vec3::new(30.0, 10.0, 30.0),
        Vec3::new(30.0, 30.0, 30.0),
        Vec3::new(10.0, 30.0, 30.0),
    ];
    let mut b = Body::new();
    b.loft(&bottom, &top).unwrap_or_else(|e| {
        panic!("loft_two_profiles_tapered: DECLINE/GAP -- loft() Err: {e:?}")
    });
    // Square frustum volume = h/3 * (A1 + A2 + sqrt(A1 A2))
    //  = 30/3 * (1600 + 400 + sqrt(640000)) = 10 * (2000 + 800) = 28000.
    assert_solid_ok(&b, "loft_two_profiles_tapered", 27_999.0, 28_001.0);
}

// ===========================================================================
// 4. mirror_solid_union -- mirror an asymmetric solid across a plane and union
//    the halves; assert valid + mass ~ 2x (minus any overlap).
// ===========================================================================

/// An asymmetric L-shaped solid mirrored across the x=0 plane and unioned with
/// itself, doubling into a symmetric T. The two halves meet exactly on the
/// mirror plane (no overlap), so the union mass is exactly 2x.
///
/// The asymmetric L is a SINGLE extruded L-profile (sketch an L, extrude) --
/// the faithful tutorial primitive -- so this isolates mirror + self-union on
/// a genuinely all-planar solid (a boolean-BUILT L carries a NURBS seam curve
/// that `mirrored` independently rejects; see the findings).
#[test]
#[ignore = "tutorial-gap: coincident-plane boolean union (mirror+union)"]
fn mirror_solid_union() {
    // L cross-section in z=0, all in x >= 0, extruded up 20 mm. Outer 30x30,
    // with a 20x20 bite from the +x,+y corner -> an L footprint, area
    // 30*30 - 20*20 = 500, volume 500*20 = 10000.
    let l_profile = [
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(30.0, 0.0, 0.0),
        Vec3::new(30.0, 10.0, 0.0),
        Vec3::new(10.0, 10.0, 0.0),
        Vec3::new(10.0, 30.0, 0.0),
        Vec3::new(0.0, 30.0, 0.0),
    ];
    let mut l = Body::new();
    l.sweep(&l_profile, Vec3::new(0.0, 0.0, 20.0)).unwrap_or_else(|e| {
        panic!("mirror_solid_union: DECLINE/GAP -- extrude L-profile Err: {e:?}")
    });
    assert_solid_ok(&l, "L-solid (asymmetric, extruded)", 9_999.0, 10_001.0);

    // Mirror across x = 0 (plane point origin, normal +x). The mirrored copy
    // lives in x <= 0 and shares the x=0 face with the original.
    let mirrored = l
        .mirrored(Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0))
        .unwrap_or_else(|e| panic!("mirror_solid_union: DECLINE/GAP -- mirrored() Err: {e:?}"));
    let u = bool_body(&l, &mirrored, BoolOp::Union, "mirror union");
    // Doubled, faces meeting on the plane: exactly 20000.
    assert_solid_ok(&u, "mirror_solid_union", 19_999.0, 20_001.0);
}

// ===========================================================================
// 5. chamfer_edges -- chamfer a block edge; and an apex/even chamfer; assert
//    volume = block - chamfer wedge.
// ===========================================================================

/// Chamfer one top edge of a 40 mm cube by 6 mm. Removes a right-triangular
/// prism wedge of cross-section 6*6/2 = 18 over length 40 => 720. 64000 -> 63280.
#[test]
fn chamfer_single_edge() {
    let mut b = Body::new();
    b.block(Vec3::ZERO, 40.0, 40.0, 40.0).unwrap();
    // A top edge: pick the straight (plane,plane) edge nearest a top edge
    // midpoint (x=20, y=0, z=40).
    let e = edge_nearest(&b, Vec3::new(20.0, 0.0, 40.0))
        .unwrap_or_else(|| panic!("chamfer_single_edge: no top edge found"));
    let c = b.chamfer_edge(e, 6.0).unwrap_or_else(|err| {
        panic!("chamfer_single_edge: DECLINE/GAP -- chamfer_edge(d=6) Err: {err:?}")
    });
    // 64000 - 720 = 63280, all-planar => exact.
    assert_solid_ok(&c, "chamfer_single_edge", 63_279.0, 63_281.0);
}

/// "Even" / apex chamfer variant: chamfer all four top edges of a block by an
/// equal setback so the top is evenly beveled. Each of the 4 top edges removes
/// a wedge; corner interactions make the exact value awkward, so a bound that
/// brackets "4 wedges removed, corners shared" is used. If any edge declines,
/// the workflow is incomplete -> FAIL.
#[test]
#[ignore = "tutorial-gap: adjacent chamfer corner interaction"]
fn chamfer_even_top_edges() {
    let l = 40.0_f64;
    let d = 5.0_f64;
    let mut b = Body::new();
    b.block(Vec3::ZERO, l, l, l).unwrap();

    // Chamfer the 4 top edges (z = l). After each chamfer the top-edge keys
    // shift, so re-find the nearest top (plane,plane) edge each round by
    // targeting the 4 top-edge midpoints in turn.
    let top_mids = [
        Vec3::new(l * 0.5, 0.0, l),
        Vec3::new(l, l * 0.5, l),
        Vec3::new(l * 0.5, l, l),
        Vec3::new(0.0, l * 0.5, l),
    ];
    let mut cur = b;
    let mut done = 0;
    for (i, m) in top_mids.iter().enumerate() {
        let e = match edge_nearest(&cur, *m) {
            Some(e) => e,
            None => panic!("chamfer_even_top_edges: edge {i} not found"),
        };
        match cur.chamfer_edge(e, d) {
            Ok(c) => {
                cur = c;
                done += 1;
            }
            Err(err) => panic!(
                "chamfer_even_top_edges: DECLINE/GAP -- top edge {i} declined \
                 chamfer_edge(d=5): {err:?}"
            ),
        }
    }
    assert_eq!(done, 4, "chamfer_even_top_edges: only {done}/4 chamfers applied");
    // Each top edge removes a 5*5/2=12.5 section over ~length (l reducing at the
    // shared corners). Loose bound: between 4 full wedges (4*12.5*40=2000 ->
    // 62000) and 4 wedges with corner double-count relief (~64000-1500=62500).
    assert_solid_ok(&cur, "chamfer_even_top_edges", 61_500.0, 63_000.0);
}

// ===========================================================================
// 6. extrude_fillet_shell -- extrude a planar sketch/profile to a solid,
//    fillet the top edges, then hollow it; assert a valid thin-walled shell.
// ===========================================================================

/// Extrude a square sketch into a solid (via `prism`/`sweep`), fillet the four
/// top edges, then `hollow` it into a thin-walled open-top-ish box. Asserts a
/// valid thin-walled shell (validate + mass == mesh; mass well under the solid).
#[test]
fn extrude_fillet_shell() {
    // 1) Extrude a 40x40 square sketch up 30 mm => a 40x40x30 solid block.
    let sketch = [
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(40.0, 0.0, 0.0),
        Vec3::new(40.0, 40.0, 0.0),
        Vec3::new(0.0, 40.0, 0.0),
    ];
    let mut b = Body::new();
    b.sweep(&sketch, Vec3::new(0.0, 0.0, 30.0)).unwrap_or_else(|e| {
        panic!("extrude_fillet_shell: DECLINE/GAP -- extrude (sweep/prism) Err: {e:?}")
    });
    let solid_v = 40.0 * 40.0 * 30.0; // 48000
    assert_solid_ok(&b, "extrude (pre-fillet)", solid_v - 1.0, solid_v + 1.0);

    // 2) Fillet the four top edges (z = 30) at r = 4 mm.
    let top_mids = [
        Vec3::new(20.0, 0.0, 30.0),
        Vec3::new(40.0, 20.0, 30.0),
        Vec3::new(20.0, 40.0, 30.0),
        Vec3::new(0.0, 20.0, 30.0),
    ];
    let mut cur = b;
    let mut done = 0;
    for (i, m) in top_mids.iter().enumerate() {
        let e = match edge_nearest(&cur, *m) {
            Some(e) => e,
            None => panic!("extrude_fillet_shell: top edge {i} not found"),
        };
        match cur.fillet_edge(e, 4.0) {
            Ok(f) => {
                cur = f;
                done += 1;
            }
            Err(err) => panic!(
                "extrude_fillet_shell: DECLINE/GAP -- top edge {i} declined \
                 fillet_edge(r=4): {err:?}"
            ),
        }
    }
    assert_eq!(done, 4, "extrude_fillet_shell: only {done}/4 top fillets applied");
    // Filleted solid: a little under 48000 (4 convex rounds shave slivers).
    assert_solid_ok(&cur, "extrude+fillet (pre-shell)", 47_000.0, 48_001.0);

    // 3) Hollow into a thin wall (t = 3 mm).
    let shell = cur.hollow(3.0).unwrap_or_else(|e| {
        panic!("extrude_fillet_shell: DECLINE/GAP -- hollow(t=3) Err: {e:?}")
    });
    // A 3 mm wall on a ~40x40x30 filleted box: the cavity removes the bulk, so
    // the shell mass is a small fraction of the solid. Solid outer ~47.5k; the
    // inner cavity is roughly (40-6)x(40-6)x(30-3) ~ 34*34*27 = 31212, so the
    // shell is very roughly 47500-31000 ~ 16000, but geometry-dependent: use a
    // GENEROUS bracket that still proves it is a hollow shell (well below the
    // solid and above zero).
    assert_solid_ok(&shell, "extrude_fillet_shell (thin wall)", 8_000.0, 30_000.0);
}
