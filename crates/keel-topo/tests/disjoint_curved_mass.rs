//! Disjoint curved-union mass: the dossier-72 oracle battery.
//!
//! A union of AABB-separated bodies short-circuits to `combine_disjoint`,
//! producing a valid disconnected (multi-shell, multi-region) body. The
//! divergence-theorem `mass_properties` MUST sum every lump's volume. The
//! confirmed defect: a MIRRORED curved lump (an improper isometry, det = -1)
//! reflects each surface frame, flipping the curved integrator's natural
//! normal `du x dv` (a pseudovector) so it points inward -- that lump's flux
//! then SUBTRACTS and the body volume collapses (sphere mirror+union read 238
//! against the true 14441). The mesh path orients curved facets from geometry
//! (`q - centre`), which does not flip, so mass and mesh disagreed exactly
//! when the frame was reflected. The fix folds the surface-frame handedness
//! into the curved integrator's orientation, so analytic mass agrees with the
//! geometry-grounded mesh for both proper and reflected frames.
//!
//! The battery (dossier 72 section 4):
//!   1. Two disjoint spheres (no mirror): additive, the det=+1 baseline.
//!   2. Two disjoint cylinders (no mirror): additivity not sphere-specific.
//!   3. Sphere mirror+union (the confirmed-bug case): DECLINE -> PASS, the
//!      mirrored lump must ADD; mass == mesh == 2 * V_sphere.
//!   4. Planar two-block disjoint union (the baseline): EXACT, bit-unchanged.
//! Plus: a standalone mirrored sphere/cylinder must compute correct analytic
//! mass (the latent single-body bug the same fix closes), and a planar
//! mirror+union (det=-1 on a block, which the planar path always handled).
#![allow(clippy::unwrap_used)]

use keel_geom::surface::Frame3;
use keel_math::vec::Vec3;
use keel_topo::Body;
use keel_topo::boolean::{BoolOp, boolean};

const PI: f64 = std::f64::consts::PI;

fn sph(center: Vec3, r: f64) -> Body {
    let mut b = Body::new();
    b.sphere(Frame3::from_z(center, Vec3::new(0.0, 0.0, 1.0)).unwrap(), r)
        .unwrap();
    b
}

fn cyl(base: Vec3, r: f64, h: f64) -> Body {
    let mut b = Body::new();
    b.cylinder(
        Frame3::from_z(base, Vec3::new(0.0, 0.0, 1.0)).unwrap(),
        r,
        h,
    )
    .unwrap();
    b
}

fn blk(o: Vec3, dx: f64, dy: f64, dz: f64) -> Body {
    let mut b = Body::new();
    b.block(o, dx, dy, dz).unwrap();
    b
}

/// `mass == mesh == closed_form` within the 2% curved band, with the
/// closed-form sum as the tight oracle and the mesh as the independent
/// cross-check (the mesh side is chordal, so the band absorbs its deficit).
fn assert_curved_triple(body: &Body, closed_form: f64, what: &str) {
    assert!(body.validate().is_ok(), "{what}: body invalid");
    let mass = body
        .mass_properties()
        .unwrap_or_else(|e| panic!("{what}: mass_properties failed: {e:?}"))
        .volume;
    let mesh = body.mesh_volume();
    let band = 2e-2 * (1.0 + closed_form);
    assert!(
        (mass - closed_form).abs() <= band,
        "{what}: analytic mass {mass} != closed-form {closed_form} (band {band})"
    );
    // The mesh is the coarse party (chordal); allow the documented curved
    // tessellation deficit (a single sphere meshes ~0.3% under per lump).
    let mesh_band = 5e-2 * (1.0 + closed_form);
    assert!(
        (mesh - closed_form).abs() <= mesh_band,
        "{what}: mesh {mesh} != closed-form {closed_form} (band {mesh_band})"
    );
    // mass and mesh must agree to the curved band (the soak's own criterion).
    let mm_band = 5e-2 * (1.0 + mass.abs());
    assert!(
        (mass - mesh).abs() <= mm_band,
        "{what}: mass {mass} != mesh {mesh} (band {mm_band})"
    );
}

/// `mass == mesh == closed_form` to 1e-9 (planar: the mesh is EXACT).
fn assert_planar_triple(body: &Body, closed_form: f64, what: &str) {
    assert!(body.validate().is_ok(), "{what}: body invalid");
    let mass = body
        .mass_properties()
        .unwrap_or_else(|e| panic!("{what}: mass_properties failed: {e:?}"))
        .volume;
    let mesh = body.mesh_volume();
    let tol = 1e-9 * (1.0 + closed_form);
    assert!(
        (mass - closed_form).abs() <= tol,
        "{what}: analytic mass {mass} != closed-form {closed_form} (exact expected)"
    );
    assert!(
        (mesh - closed_form).abs() <= tol,
        "{what}: mesh {mesh} != closed-form {closed_form} (exact expected)"
    );
    assert!(
        (mass - mesh).abs() <= tol,
        "{what}: mass {mass} != mesh {mesh} (planar must be exact)"
    );
}

#[test]
fn two_disjoint_spheres_mass_is_sum() {
    // Case 1: AABB-separated, no mirror. The det=+1 multi-lump baseline.
    let r1 = 7.0;
    let r2 = 5.0;
    let a = sph(Vec3::ZERO, r1);
    let b = sph(Vec3::new(40.0, 0.0, 0.0), r2);
    let u = boolean(&a, &b, BoolOp::Union, 1e-7).expect("two-sphere union");
    assert!(
        u.faults.is_empty(),
        "two-sphere union faulted: {:?}",
        u.faults
    );
    let closed = 4.0 / 3.0 * PI * (r1 * r1 * r1 + r2 * r2 * r2);
    assert_curved_triple(&u.body, closed, "two disjoint spheres");
    assert_eq!(
        u.body.connected_components().len(),
        2,
        "two disjoint spheres must be 2 components"
    );
}

#[test]
fn two_disjoint_cylinders_mass_is_sum() {
    // Case 2: a second curved primitive, proper isometry. Additivity is not
    // sphere-specific.
    let (r1, h1) = (4.0, 6.0);
    let (r2, h2) = (3.0, 8.0);
    let a = cyl(Vec3::new(0.0, 0.0, -h1 * 0.5), r1, h1);
    let b = cyl(Vec3::new(30.0, 0.0, -h2 * 0.5), r2, h2);
    let u = boolean(&a, &b, BoolOp::Union, 1e-7).expect("two-cylinder union");
    assert!(
        u.faults.is_empty(),
        "two-cylinder union faulted: {:?}",
        u.faults
    );
    let closed = PI * (r1 * r1 * h1 + r2 * r2 * h2);
    assert_curved_triple(&u.body, closed, "two disjoint cylinders");
}

#[test]
fn standalone_mirrored_sphere_mass_is_correct() {
    // The latent single-body bug the same fix closes: a mirrored sphere's
    // analytic mass was NEGATIVE (mass_properties errored "non-positive
    // volume") because the reflected frame flipped du x dv inward, while the
    // mesh stayed correct. Now both agree.
    let r = 9.0;
    let s = sph(Vec3::ZERO, r);
    let m = s
        .mirrored(Vec3::new(20.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0))
        .expect("mirror sphere");
    assert!(m.validate().is_ok(), "mirrored sphere invalid");
    let exact = 4.0 / 3.0 * PI * r * r * r;
    let mass = m
        .mass_properties()
        .expect("mirrored sphere mass must compute (was non-positive)")
        .volume;
    assert!(
        (mass - exact).abs() <= 2e-2 * (1.0 + exact),
        "mirrored sphere analytic mass {mass} != {exact}"
    );
    let mesh = m.mesh_volume();
    assert!(
        (mass - mesh).abs() <= 5e-2 * (1.0 + exact),
        "mirrored sphere mass {mass} != mesh {mesh}"
    );
}

#[test]
fn standalone_mirrored_cylinder_mass_is_correct() {
    // A mirrored cylinder's analytic mass must also be correct (the lateral
    // takes the handedness fold; the caps are planar and self-correct).
    let (r, h) = (3.0, 5.0);
    let c = cyl(Vec3::new(2.0, 0.0, 0.0), r, h);
    let m = c
        .mirrored(Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0))
        .expect("mirror cylinder");
    assert!(m.validate().is_ok(), "mirrored cylinder invalid");
    let exact = PI * r * r * h;
    let mass = m
        .mass_properties()
        .expect("mirrored cylinder mass must compute")
        .volume;
    assert!(
        (mass - exact).abs() <= 2e-2 * (1.0 + exact),
        "mirrored cylinder analytic mass {mass} != {exact}"
    );
}

#[test]
fn sphere_mirror_union_mass_is_sum_not_collapse() {
    // Case 3: THE load-bearing case (realsoak seed 11400715918834829910 in
    // miniature). A sphere mirrored across a far plane, then unioned: two
    // disjoint curved lumps, one a reflected copy. Before the fix the
    // mirrored lump's flux subtracted (mass ~ V_a - V_b ~ 0, declined by the
    // floor gate); after, it ADDS (mass == mesh == 2 * V_sphere).
    let r = 10.0;
    let s = sph(Vec3::ZERO, r);
    let mirrored = s
        .mirrored(Vec3::new(15.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0))
        .expect("mirror sphere");
    let u = boolean(&s, &mirrored, BoolOp::Union, 1e-7)
        .expect("sphere mirror+union must NOT decline (was floored)");
    assert!(
        u.faults.is_empty(),
        "sphere mirror+union faulted: {:?}",
        u.faults
    );
    let v_sphere = 4.0 / 3.0 * PI * r * r * r;
    let closed = 2.0 * v_sphere; // a reflection preserves volume
    assert_curved_triple(&u.body, closed, "sphere mirror+union");
    assert_eq!(
        u.body.connected_components().len(),
        2,
        "sphere mirror+union must be 2 disjoint lumps"
    );
}

#[test]
fn planar_two_block_disjoint_union_stays_exact() {
    // Case 4: the no-regression baseline. A planar two-block disjoint union
    // computes mass == mesh EXACTLY today and must stay exact (the det=+1
    // path is bit-unchanged; the handedness fold only touches curved faces).
    let a = blk(Vec3::ZERO, 2.0, 3.0, 4.0);
    let b = blk(Vec3::new(20.0, 0.0, 0.0), 5.0, 2.0, 3.0);
    let u = boolean(&a, &b, BoolOp::Union, 1e-7).expect("two-block union");
    assert!(
        u.faults.is_empty(),
        "two-block union faulted: {:?}",
        u.faults
    );
    let closed = 2.0 * 3.0 * 4.0 + 5.0 * 2.0 * 3.0;
    assert_planar_triple(&u.body, closed, "planar two-block disjoint union");
    assert_eq!(
        u.body.connected_components().len(),
        2,
        "planar two-block union must be 2 components"
    );
}

#[test]
fn planar_block_mirror_union_stays_exact() {
    // A planar mirror+union (det=-1 on a block): the planar integrator always
    // handled this (it folds the loop winding, which the reflection also
    // flips), so it must remain EXACT and not regress under the curved fix.
    let a = blk(Vec3::ZERO, 4.0, 4.0, 4.0);
    let mirrored = a
        .mirrored(Vec3::new(10.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0))
        .expect("mirror block");
    let u = boolean(&a, &mirrored, BoolOp::Union, 1e-7).expect("block mirror+union");
    assert!(
        u.faults.is_empty(),
        "block mirror+union faulted: {:?}",
        u.faults
    );
    let closed = 2.0 * (4.0 * 4.0 * 4.0);
    assert_planar_triple(&u.body, closed, "planar block mirror+union");
}
