//! Analytic curved-face mass is GENERALIZED-WINDING-NUMBER consistent on a
//! cylinder whose lateral is split into axial bands (realsoak seed
//! 11400715918834826719, the compound-operand `mass != mesh` decline).
//!
//! ## What seed 719 is, and what it is NOT (the proven characterization)
//!
//! Seed 719's `extrudeU` assembles a body that the curved post-condition gate
//! declines with `AssemblyFailed("degenerate (mass != mesh)")`: the body is
//! reported watertight (`mesh_open_ratio` 0.00), its `tessellated_volume` and
//! `mesh_volume` AGREE at ~6647, yet the analytic `mass_properties` reads ~5480
//! (an ~18% gap). The naive reading (dossier 73 lumped it with dossier 71) is
//! that the green-slab UNDER-integrates a curved face and the mesh is the truth.
//!
//! That reading is WRONG, and this was settled with an INDEPENDENT witness: a
//! generalized-winding-number Monte-Carlo volume. GWN sums signed solid angles
//! (Van Oosterom-Strackee), an integrator INDEPENDENT of the divergence/flux
//! volume that the analytic mass AND the mesh tetra-sum both use, so it
//! adjudicates the two. On seed 719's assembled body the GWN volume is ~5474 --
//! it matches the ANALYTIC mass (~5480) to 0.1%, NOT the mesh (~6647). Three
//! independent facts agree:
//!   * the body has NINE radial-1 edges (a dangling open box-protrusion sheet
//!     grafted onto a closed cylinder; `mesh_open_ratio` 0.00 is a coincidental
//!     net-area-vector cancellation, a false negative for watertightness);
//!   * GWN ~ 0 at points inside the box protrusion (it encloses no solid there);
//!   * each cylinder band's analytic flux equals its OWN tessellated x-flux to
//!     < 0.2% (the green-slab reads every band correctly).
//! So the analytic mass is CORRECT (it integrates the only genuinely-enclosed
//! region, the cylinder), and the MESH over-reads the malformed body via the
//! signed-tetra sum. This is the SAME shape as dossier 71's own resolution: the
//! integrator was correct and the SURGERY (there) / ASSEMBLER (here) handed it a
//! non-watertight body. It is NOT a `massprops` integration bug, so forcing the
//! analytic mass up to the mesh would report a WRONG volume for a WRONG body and
//! break DECLINE-never-WRONG. The gate's decline is correct.
//!
//! These tests lock in the load-bearing invariant: the curved-face mass
//! integrator is EXACT and GWN-self-consistent on the multi-axial-band cylinder
//! configuration seed 719 exercises (a cylinder with material added/removed that
//! splits its lateral into bands), so the analytic mass is the trusted,
//! self-consistent value whenever the body is genuinely watertight.
#![allow(clippy::unwrap_used)]

use keel_geom::surface::Frame3;
use keel_math::vec::Vec3;
use keel_topo::Body;
use keel_topo::boolean::{BoolOp, boolean};

fn cyl(pos: Vec3, r: f64, h: f64) -> Body {
    let mut b = Body::new();
    b.cylinder(Frame3::from_z(pos, Vec3::new(0.0, 0.0, 1.0)).unwrap(), r, h)
        .unwrap();
    b
}

/// Independent volume witness: Monte-Carlo integrate generalized-winding-number
/// membership (`gwn > 0.5`) over the body's tessellated AABB. GWN is independent
/// of the divergence/flux volume the analytic mass and the mesh both use, so it
/// adjudicates them. Deterministic xorshift sampler (no rng dependency).
fn gwn_volume(b: &Body, samples: usize) -> f64 {
    let tris = b.facets(None);
    assert!(!tris.is_empty(), "empty tessellation");
    let mut lo = Vec3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
    let mut hi = Vec3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for t in &tris {
        for p in t {
            lo = Vec3::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
            hi = Vec3::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
        }
    }
    let mut s: u64 = 0x243F_6A88_85A3_08D3;
    let mut nxt = || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        (s >> 11) as f64 / (1u64 << 53) as f64
    };
    let mut inside = 0usize;
    for _ in 0..samples {
        let p = Vec3::new(
            lo.x + (hi.x - lo.x) * nxt(),
            lo.y + (hi.y - lo.y) * nxt(),
            lo.z + (hi.z - lo.z) * nxt(),
        );
        if b.generalized_winding_number(p) > 0.5 {
            inside += 1;
        }
    }
    let boxv = (hi.x - lo.x) * (hi.y - lo.y) * (hi.z - lo.z);
    boxv * inside as f64 / samples as f64
}

// GWN membership is O(triangles) per probe, so keep the sample count modest:
// at ~50k samples with inside-fraction ~0.5 the volume MC relative error is
// ~0.6%, comfortably inside the 1.5% acceptance band, and the tube's EXACT
// closed-form check (1e-6) carries the precision the GWN witness only confirms.
const SAMPLES: usize = 50_000;

/// A coaxial through-bore splits the cylinder lateral into an inner + outer wall
/// (a holed band): the analytic green-slab integrates the tube EXACTLY (the
/// closed-form `pi (R^2 - r^2) h`), and the independent GWN witness confirms it.
/// The mesh is chord-low here, so this is the clean inverse of the seed-719
/// premise -- on a genuinely watertight banded cylinder the ANALYTIC mass is the
/// exact truth and the tessellation is the approximation.
#[test]
fn tube_mass_is_exact_and_gwn_consistent() {
    let (big_r, small_r, h) = (4.0, 2.0, 10.0);
    let outer = cyl(Vec3::new(0.0, 0.0, -5.0), big_r, h);
    let bore = cyl(Vec3::new(0.0, 0.0, -6.0), small_r, h + 2.0);
    let tube = boolean(&outer, &bore, BoolOp::Difference, 1e-7)
        .expect("bore difference declined")
        .body;
    assert!(tube.validate().is_ok(), "tube invalid");

    let mass = tube.mass_properties().expect("tube mass declined").volume;
    let exact = core::f64::consts::PI * (big_r * big_r - small_r * small_r) * h;
    // The green-slab integrates the holed band to the closed form EXACTLY.
    assert!(
        (mass - exact).abs() < 1e-6 * (1.0 + exact),
        "tube analytic mass {mass} != closed form {exact}"
    );
    // The INDEPENDENT GWN witness agrees with the analytic mass (1.5% MC band).
    let gwn = gwn_volume(&tube, SAMPLES);
    assert!(
        (mass - gwn).abs() < 1.5e-2 * (1.0 + gwn),
        "tube analytic mass {mass} not GWN-consistent (gwn {gwn}, gap {:.2}%)",
        100.0 * (mass - gwn).abs() / gwn
    );
}

/// A stepped shaft (a thin tall cylinder unioned with a short fat coaxial
/// collar) splits the outer lateral into axial bands -- the seed-719 shape
/// (material added to a cylinder, banding its wall), but assembled WATERTIGHT.
/// The analytic mass must equal the closed-form stacked volume AND the GWN
/// witness, proving the band integrator is correct when the body is sound.
#[test]
fn stepped_shaft_union_mass_is_gwn_consistent() {
    let shaft = cyl(Vec3::new(0.0, 0.0, -5.0), 2.0, 10.0);
    let collar = cyl(Vec3::new(0.0, 0.0, -1.0), 4.0, 2.0);
    let stepped = boolean(&shaft, &collar, BoolOp::Union, 1e-7)
        .expect("stepped union declined")
        .body;
    assert!(stepped.validate().is_ok(), "stepped shaft invalid");

    let mass = stepped.mass_properties().expect("mass declined").volume;
    // Closed form: shaft volume + the collar's annular surplus outside the shaft.
    let pi = core::f64::consts::PI;
    let exact = pi * 2.0 * 2.0 * 10.0 + pi * (4.0 * 4.0 - 2.0 * 2.0) * 2.0;
    assert!(
        (mass - exact).abs() < 1e-6 * (1.0 + exact),
        "stepped analytic mass {mass} != closed form {exact}"
    );
    let gwn = gwn_volume(&stepped, SAMPLES);
    assert!(
        (mass - gwn).abs() < 1.5e-2 * (1.0 + gwn),
        "stepped analytic mass {mass} not GWN-consistent (gwn {gwn}, gap {:.2}%)",
        100.0 * (mass - gwn).abs() / gwn
    );
}

/// Integrator-correctness floor that must not regress: a plain full cylinder
/// integrates to the closed-form volume and is GWN-consistent.
#[test]
fn full_cylinder_mass_is_gwn_consistent() {
    let (r, h) = (3.0, 7.0);
    let c = cyl(Vec3::new(1.0, -2.0, 0.0), r, h);
    let mass = c.mass_properties().unwrap().volume;
    let exact = core::f64::consts::PI * r * r * h;
    assert!((mass - exact).abs() < 1e-6, "cyl mass {mass} != {exact}");
    let gwn = gwn_volume(&c, SAMPLES);
    assert!(
        (mass - gwn).abs() < 1.5e-2 * (1.0 + gwn),
        "cyl mass {mass} not GWN-consistent (gwn {gwn})"
    );
}
