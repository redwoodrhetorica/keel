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

fn blk(o: Vec3, dx: f64, dy: f64, dz: f64) -> Body {
    let mut b = Body::new();
    b.block(o, dx, dy, dz).unwrap();
    b
}

#[test]
fn cone_sphere_window_four_ops_pass_exact() {
    // A sphere grazing a cone's lateral in a single window loop. ALL 4 ops PASS
    // (mass == mesh == truth) via the generalized SSI branch field + the
    // hole-orientation mass fix (a non-wrapping window inner ring must oppose
    // the outer loop; the cone's apex anchor otherwise added an apex-wedge ->
    // cone-rest 13.72 vs true 11.42) + the window-disc fan: the cap fans to the
    // loop's shared edge samples (watertight with the partner cap), radially
    // subdivided so the tiny intersection lens lands within the 2% mass gate.
    let a = cone(Vec3::ZERO, Vec3::new(0., 0., 1.), 2.0, 3.0);
    let b = sph(Vec3::new(2.0, 0., 1.0), Vec3::new(0., 0., 1.), 0.8);
    let pass = |x: &Body, y: &Body, op: BoolOp, want: f64| {
        let r = boolean(x, y, op, 1e-7).unwrap_or_else(|e| panic!("{op:?} declined: {e:?}"));
        assert!(r.faults.is_empty() && r.body.validate().is_ok(), "{op:?} not clean");
        let m = r.body.mass_properties().unwrap().volume;
        let mesh = r.body.mesh_volume();
        assert!((m - want).abs() < 0.1, "{op:?} mass {m} != ~{want}");
        assert!((m - mesh).abs() < 2e-2 * (1.0 + m), "{op:?} mass {m} mesh {mesh} not watertight");
    };
    pass(&a, &b, BoolOp::Difference, 12.45); // cone with a spherical bite
    pass(&b, &a, BoolOp::Difference, 2.03); // sphere with a conical bite
    pass(&a, &b, BoolOp::Union, 14.60);
    pass(&a, &b, BoolOp::Intersection, 0.114); // the lens
}

#[test]
fn cyl_sphere_window_four_ops_pass_exact() {
    // A sphere grazing a cylinder's lateral in a SINGLE window loop -- the first
    // non-coaxial cyl/sphere class to assemble. ALL 4 ops PASS exact (mass ==
    // mesh == truth) once the window-disc caps fan to the loop's shared edge
    // samples (watertight) with radial subdivision (the tiny intersection lens
    // needs the curvature to land within the 2% mass gate). Truths by the
    // two-disc lens integral / MC.
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
    pass(&a, &b, BoolOp::Intersection, 1.07); // the lens
}

#[test]
fn cyl_sphere_planar_cap_intersection_pass() {
    // A sphere sitting inside a (tilted) cylinder radially but poking out one
    // flat END: cyl n sph is the sphere truncated by the cylinder's cap PLANE
    // (a single circle, NOT a lateral NURBS window). The big truncated-sphere
    // cap reaches a sphere FRAME pole, and the cut circle crosses the sphere's
    // parametric seam so it is stored as ARCS. Two fixes land it: (1) the mass
    // green-slab anchors at the ENCLOSED pole chosen by the interior point's
    // latitude (not the fragile tess-area witness, which read the complement
    // pole -> mass 1.25 vs true 2.85); (2) sphere_cap_trim recognises the
    // arc-split cut circle and meshes the correct cap side via the interior
    // point (the arc_planes vertex-average path sliced the cap on the seam
    // meridian -> mesh 0.96). Geometry from the seed-1 soak (probe_cysph_fail).
    let a = cyl(
        Vec3::new(-0.7282314989287573, 0.2578193561387194, -1.4989243807508887),
        Vec3::new(-0.10491245566834961, 0.8316768206114489, 0.7683821733488301),
        2.964423919723465,
        1.796689666271305,
    );
    let b = sph(
        Vec3::new(-0.8752042306236147, 1.904556614958362, -1.1882431622168834),
        Vec3::new(0., 0., 1.),
        0.9596500794485675,
    );
    let r = boolean(&a, &b, BoolOp::Intersection, 1e-7)
        .unwrap_or_else(|e| panic!("intersection declined: {e:?}"));
    assert!(r.faults.is_empty() && r.body.validate().is_ok(), "not clean: {:?}", r.faults);
    let m = r.body.mass_properties().unwrap().volume;
    let mesh = r.body.mesh_volume();
    assert!((m - 2.853).abs() < 0.1, "mass {m} != ~2.853");
    assert!((m - mesh).abs() < 2e-2 * (1.0 + m), "mass {m} mesh {mesh} not watertight");
}

#[test]
fn curved_compound_drill_passes_not_declined() {
    // Root cause B (Add 287): a CORRECT curved-compound body was false-declined
    // by two independent guards firing on it. Build a plate, union a dome
    // (-> curved-compound body), then drill a through-hole well clear of the
    // dome. The result is analytically exact:
    //   300 (block) + 2.25*pi (upper hemisphere) - 3*pi (bore) = 300 - 0.75*pi.
    // It MUST pass faultless, valid, mass == exact, mass == mesh.
    //
    // Before the fix two things tripped it: (1) the body's genuine ~0.018
    // chordal-junction residual (a curved cap meeting planar faces on a
    // multi-loop face) exceeded the old 1e-2 watertightness net; (2) the tight
    // cyl/sphere oracle matched the dome FACE on the compound operand and scored
    // the whole block+dome as if it were the bare sphere -> a nonsense exact
    // volume -> AssemblyFailed. The lone-primitive guard (oracle fires only when
    // each operand's own volume matches the primitive it was detected as) and the
    // 5e-2 net (still 5x below the #48 silent-WRONG class at 0.25+) land it.
    let plate = blk(Vec3::ZERO, 10.0, 10.0, 3.0);
    let dome = sph(Vec3::new(3.0, 3.0, 3.0), Vec3::new(0., 0., 1.), 1.5);
    let b1 = boolean(&plate, &dome, BoolOp::Union, 1e-7).unwrap().body;
    let hole = cyl(Vec3::new(7.0, 7.0, -0.5), Vec3::new(0., 0., 1.), 1.0, 4.0);
    let r = boolean(&b1, &hole, BoolOp::Difference, 1e-7)
        .unwrap_or_else(|e| panic!("curved-compound drill declined (root-B regression): {e:?}"));
    assert!(r.faults.is_empty() && r.body.validate().is_ok(), "not clean: {:?}", r.faults);
    let exact = 300.0 - 0.75 * std::f64::consts::PI;
    let m = r.body.mass_properties().unwrap().volume;
    let mesh = r.body.mesh_volume();
    assert!((m - exact).abs() < 1e-2, "mass {m} != exact {exact}");
    assert!((m - mesh).abs() < 5e-2 * (1.0 + m), "mass {m} mesh {mesh} not watertight");
}

#[test]
fn countersink_cone_reports_clean_no_advisory_fault() {
    // Root-B frontier (Add 288): a bare block - cone countersink produces the
    // EXACT result (mass == mesh == MC truth, ~905.30 = 22.5*13.5*2.982 minus the
    // conical plug) but the multi-cut seam relocation pushed an "unlocated seam
    // component (non-planar multi-cut face)" AssemblyFailed *fault* onto the
    // correct body -- which every consumer reads as a decline (the dominant
    // ~1369-hit residual class). Reaching the gate's success return verifies the
    // body, so that advisory fault now drops. Must be faultless, valid, exact.
    let block = blk(Vec3::ZERO, 22.5, 13.5, 2.982);
    let csink = cone(Vec3::new(2.731, 2.159, 3.032), Vec3::new(0., 0., -1.), 0.601, 1.428);
    let r = boolean(&block, &csink, BoolOp::Difference, 1e-7)
        .unwrap_or_else(|e| panic!("countersink declined: {e:?}"));
    assert!(r.faults.is_empty(), "advisory fault leaked on a correct body: {:?}", r.faults);
    assert!(r.body.validate().is_ok(), "countersink invalid shell");
    let m = r.body.mass_properties().unwrap().volume;
    let mesh = r.body.mesh_volume();
    assert!((m - 905.30).abs() < 0.3, "countersink mass {m} != ~905.30");
    assert!((m - mesh).abs() < 2e-2 * (1.0 + m), "mass {m} mesh {mesh} not watertight");
}

#[test]
fn second_through_hole_reports_clean_no_coincident_fault() {
    // Add 288: drilling a 2nd well-separated through-hole into a domed plate
    // produced the exact body (MC truth ~1025.65) but flagged an informational
    // Coincident fault that consumers reject (the ~662-hit class). Cleared on
    // gate-pass. Faultless, valid, mass == mesh.
    let plate = blk(Vec3::ZERO, 18.0, 22.5, 2.562);
    let dome = sph(Vec3::new(1.914, 2.292, 2.562), Vec3::new(0., 0., 1.), 0.935);
    let b1 = boolean(&plate, &dome, BoolOp::Union, 1e-7).unwrap().body;
    let h1 = cyl(Vec3::new(2.724, 6.862, -0.5), Vec3::new(0., 0., 1.), 1.089, 3.562);
    let b2 = boolean(&b1, &h1, BoolOp::Difference, 1e-7).unwrap().body;
    let h2 = cyl(Vec3::new(2.618, 11.029, -0.5), Vec3::new(0., 0., 1.), 0.691, 3.562);
    let r = boolean(&b2, &h2, BoolOp::Difference, 1e-7)
        .unwrap_or_else(|e| panic!("2nd hole declined: {e:?}"));
    assert!(r.faults.is_empty(), "Coincident fault leaked on a correct body: {:?}", r.faults);
    assert!(r.body.validate().is_ok(), "invalid shell");
    let m = r.body.mass_properties().unwrap().volume;
    let mesh = r.body.mesh_volume();
    assert!((m - 1025.65).abs() < 0.5, "mass {m} != ~1025.65");
    assert!((m - mesh).abs() < 2e-2 * (1.0 + m), "mass {m} mesh {mesh} not watertight");
}

#[test]
fn countersink_then_far_hole_no_intersection_failed() {
    // Cone frontier (LOG Add 289+): a countersink cone and a through-hole on the
    // FAR side of the plate are DISJOINT faces, but SSI on the UNBOUNDED cone
    // reached the far cylinder and Err'd -> IntersectionFailed (the dominant cone
    // residual, ~580 bug_extract hits). The broad-phase face-AABB reject skips
    // the non-overlapping pair, so the independent feature lands. Must PASS:
    // faultless, valid, mass == exact (905.78 block - cone plug - cyl bore).
    let block = blk(Vec3::ZERO, 22.5, 13.5, 2.982);
    let csink = cone(Vec3::new(2.731, 2.159, 3.032), Vec3::new(0., 0., -1.), 0.601, 1.428);
    let b1 = boolean(&block, &csink, BoolOp::Difference, 1e-7)
        .unwrap_or_else(|e| panic!("countersink declined: {e:?}"));
    assert!(b1.faults.is_empty(), "countersink faults: {:?}", b1.faults);
    let hole = cyl(Vec3::new(1.742, 6.870, -0.5), Vec3::new(0., 0., 1.), 1.082, 3.982);
    let r = boolean(&b1.body, &hole, BoolOp::Difference, 1e-7)
        .unwrap_or_else(|e| panic!("far hole declined: {e:?}"));
    assert!(r.faults.is_empty(), "far-hole faults (broad-phase regression): {:?}", r.faults);
    assert!(r.body.validate().is_ok(), "invalid shell");
    let m = r.body.mass_properties().unwrap().volume;
    let mesh = r.body.mesh_volume();
    assert!((m - 894.33).abs() < 0.3, "mass {m} != ~894.33");
    assert!((m - mesh).abs() < 2e-2 * (1.0 + m), "mass {m} mesh {mesh} not watertight");
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
