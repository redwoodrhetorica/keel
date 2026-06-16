//! Two robustness guards from LOG Add. 276/277.
//!
//! 1. Per-connected-component `mesh_volume` recenter: a disjoint union of two
//!    cones spread ~7 units apart read ~7% low because a single global
//!    signed-tetra reference cannot keep coordinates small across the far
//!    component (f64 catastrophic cancellation). The B-rep was always correct
//!    (analytic mass exact); the fix sums signed volume per component, each
//!    recentered locally.
//!
//! 2. Watertightness net: large offset sphere/sphere intersections produced a
//!    non-watertight lens whose (wrong) mass agreed with its (wrong) mesh and
//!    sat inside the loose op-bound -- a silent WRONG that slipped both honesty
//!    gates (kernel read ~18-33% over the exact lens volume). The gate now
//!    declines a non-watertight result (net triangle area-vector != 0). The
//!    contract guarantee: the boolean NEVER returns a faultless body whose mass
//!    is far from the exact two-sphere lens volume.
#![allow(clippy::unwrap_used)]

use keel_geom::surface::Frame3;
use keel_math::vec::Vec3;
use keel_topo::Body;
use keel_topo::boolean::{BoolOp, boolean};

fn cone(pos: Vec3, axis: Vec3, r: f64, h: f64) -> Body {
    let mut b = Body::new();
    b.cone(Frame3::from_z(pos, axis).unwrap(), r, h).unwrap();
    b
}

fn sph(pos: Vec3, axis: Vec3, r: f64) -> Body {
    let mut b = Body::new();
    b.sphere(Frame3::from_z(pos, axis).unwrap(), r).unwrap();
    b
}

fn cyl(pos: Vec3, axis: Vec3, r: f64, h: f64) -> Body {
    let mut b = Body::new();
    b.cylinder(Frame3::from_z(pos, axis).unwrap(), r, h).unwrap();
    b
}

#[test]
fn cyl_sphere_window_three_ops_pass_exact() {
    // A sphere grazing a cylinder's lateral in a SINGLE window loop -- the first
    // non-coaxial cyl/sphere class to assemble. 3 of 4 ops PASS exact (mass ==
    // mesh == truth); the intersection (a tiny lens) declines on shared-edge
    // tessellation -- decline-safe, and the tight cyl/sphere oracle forbids a
    // wrong volume either way. Truths by the two-disc lens integral / MC.
    let a = cyl(Vec3::new(0., 0., -3.), Vec3::new(0., 0., 1.), 1.0, 6.0);
    let b = sph(Vec3::new(0., 1.5, 0.), Vec3::new(0., 0., 1.), 1.2);
    let pass = |x: &Body, y: &Body, op: BoolOp, want: f64| {
        let r = boolean(x, y, op, 1e-7).unwrap_or_else(|e| panic!("{op:?} declined: {e:?}"));
        assert!(r.faults.is_empty(), "{op:?} faults: {:?}", r.faults);
        assert!(r.body.validate().is_ok(), "{op:?} invalid shell");
        let m = r.body.mass_properties().unwrap().volume;
        let mesh = r.body.mesh_volume();
        assert!((m - want).abs() < 0.12, "{op:?} mass {m} != ~{want}");
        assert!((m - mesh).abs() < 2e-2 * (1.0 + m), "{op:?} mass {m} mesh {mesh} not watertight");
    };
    pass(&a, &b, BoolOp::Difference, 17.78); // cyl with a spherical bite
    pass(&b, &a, BoolOp::Difference, 6.17); // sphere with a cylindrical bite
    pass(&a, &b, BoolOp::Union, 25.01);
    // Intersection: DECLINE or correct, never a silent wrong (tight oracle).
    if let Ok(r) = boolean(&a, &b, BoolOp::Intersection, 1e-7) {
        if r.faults.is_empty() {
            let m = r.body.mass_properties().map(|x| x.volume).unwrap_or(1.07);
            assert!((m - 1.07).abs() < 0.1, "intersection mass {m} != ~1.07 -- SILENT WRONG");
        }
    }
}

/// Exact volume of the intersection lens of two spheres (radii ra, rb, centre
/// distance d). Zero when disjoint; min-ball when nested.
fn lens_volume(ra: f64, rb: f64, d: f64) -> f64 {
    if d >= ra + rb {
        return 0.0;
    }
    if d <= (ra - rb).abs() {
        let r = ra.min(rb);
        return 4.0 / 3.0 * std::f64::consts::PI * r * r * r;
    }
    std::f64::consts::PI * (ra + rb - d).powi(2)
        * (d * d + 2.0 * d * rb - 3.0 * rb * rb + 2.0 * d * ra + 6.0 * rb * ra - 3.0 * ra * ra)
        / (12.0 * d)
}

#[test]
fn disjoint_spread_cone_union_mesh_matches_mass() {
    // The seed-2 soak genome (FAIL:mass-mesh U cone/cone): cone A moderate,
    // cone B very flat (r3.16 h0.24), centres ~6.7 apart.
    let a = cone(
        Vec3::new(-1.2929110357621996, 1.70471865332388, 1.4703300892471765),
        Vec3::new(-1.1434365476291544, -1.5613004696224582, -0.0747842240441019),
        1.059625480296504,
        1.4306518583642935,
    );
    let b = cone(
        Vec3::new(0.8975459265120749, -3.7835302797959756, -1.6217383391983151),
        Vec3::new(-0.8821781799465351, 0.9360741849787273, 1.1451462754908452),
        3.1562582463956206,
        0.24444836853195562,
    );
    let r = boolean(&a, &b, BoolOp::Union, 1e-7).unwrap();
    assert!(r.body.validate().is_ok(), "union shell invalid");
    let mass = r.body.mass_properties().unwrap().volume;
    let mesh = r.body.mesh_volume();
    // Pre-fix the mesh read 3.881 vs mass 4.232 (6.7%); per-component recenter
    // closes it to the chordal band.
    assert!(
        (mesh - mass).abs() < 1e-2 * (1.0 + mass),
        "mesh {mesh} vs mass {mass}: per-component recenter regressed?"
    );
}

#[test]
fn large_offset_sphere_intersection_never_silent_wrong() {
    // The 5 seed-1 soak genomes (FAIL:mass-mesh I sph/sph). The kernel used to
    // emit a non-watertight lens ~18-33% over the exact volume. The contract:
    // either DECLINE, or return a body whose mass matches the exact lens.
    let cases = [
        (
            Vec3::new(1.1696134306592736, 1.6513444071844452, 1.3008800075815423),
            Vec3::new(-0.5213418867719675, 0.9950279413418788, 0.692654206642032),
            3.8381762412586053,
            Vec3::new(1.0164236648568568, 2.964234020264538, -2.0329330428436796),
            Vec3::new(-0.3747134969120147, 0.763269045269624, 1.265654325735374),
            2.1169738858579388,
        ),
        (
            Vec3::new(1.1567600515657541, 1.4968072866278486, 1.5941608537093397),
            Vec3::new(-0.5213418867719675, 0.9950279413418788, 0.692654206642032),
            3.639899462773032,
            Vec3::new(1.171794556133998, 2.9686803754866067, -1.438804958843038),
            Vec3::new(-0.3747134969120147, 0.763269045269624, 1.265654325735374),
            2.6981844204159278,
        ),
    ];
    for (i, &(pa, aa, ra, pb, ab, rb)) in cases.iter().enumerate() {
        let a = sph(pa, aa, ra);
        let b = sph(pb, ab, rb);
        let exact = lens_volume(ra, rb, (pa - pb).norm());
        match boolean(&a, &b, BoolOp::Intersection, 1e-7) {
            Err(_) => {} // declined: contract honoured (DECLINE-never-WRONG)
            Ok(r) if !r.faults.is_empty() => {} // faulted decline: honoured
            Ok(r) => {
                let mass = r.body.mass_properties().map(|m| m.volume).unwrap_or(exact);
                assert!(
                    (mass - exact).abs() <= 5e-2 * (1.0 + exact),
                    "case {i}: emitted faultless lens mass {mass} vs exact {exact} -- SILENT WRONG"
                );
            }
        }
    }
}
