#![no_main]
use keel_topo::Body;
use keel_topo::pmc::Containment;
use libfuzzer_sys::fuzz_target;

// PMC must agree with the implicit-form sign oracle (away from the
// tolerance band) on randomly parameterized primitives, and never
// panic; ladder exhaustion must surface as a clean error.
fuzz_target!(|data: (u8, f64, f64, f64, f64, f64, f64, f64)| {
    let (kind, a, bp, c, px, py, pz, extra) = data;
    let finite = [a, bp, c, px, py, pz, extra].iter().all(|x| x.is_finite());
    if !finite {
        return;
    }
    let clampp = |x: f64, lo: f64, hi: f64| x.abs().clamp(lo, hi);
    let p = keel_math::vec::Vec3::new(
        px.clamp(-100.0, 100.0),
        py.clamp(-100.0, 100.0),
        pz.clamp(-100.0, 100.0),
    );
    let frame = match keel_geom::surface::Frame3::from_z(
        keel_math::vec::Vec3::new(a.clamp(-10.0, 10.0), bp.clamp(-10.0, 10.0), 0.0),
        keel_math::vec::Vec3::new(0., 0., 1.),
    ) {
        Ok(f) => f,
        Err(_) => return,
    };
    let mut body = Body::new();
    let surf = match kind % 3 {
        0 => {
            let r = clampp(c, 0.1, 20.0);
            if body.sphere(frame.clone(), r).is_err() {
                return;
            }
            keel_geom::surface::Surface3::Sphere(
                keel_geom::surface::Sphere3::new(frame, r).expect("validated"),
            )
        }
        1 => {
            let major = clampp(c, 1.0, 20.0);
            let minor = clampp(extra, 0.05, 0.9) * major;
            if body.torus(frame.clone(), major, minor).is_err() {
                return;
            }
            keel_geom::surface::Surface3::Torus(
                keel_geom::surface::Torus3::new(frame, major, minor).expect("validated"),
            )
        }
        _ => {
            let dx = clampp(a, 0.1, 20.0);
            let dy = clampp(bp, 0.1, 20.0);
            let dz = clampp(c, 0.1, 20.0);
            if body
                .block(keel_math::vec::Vec3::ZERO, dx, dy, dz)
                .is_err()
            {
                return;
            }
            // Block oracle handled separately below.
            let inside = p.x > 0.0
                && p.x < dx
                && p.y > 0.0
                && p.y < dy
                && p.z > 0.0
                && p.z < dz;
            let on_band = [p.x, p.x - dx].iter().any(|d| d.abs() < 1e-6)
                || [p.y, p.y - dy].iter().any(|d| d.abs() < 1e-6)
                || [p.z, p.z - dz].iter().any(|d| d.abs() < 1e-6);
            if on_band {
                return;
            }
            match body.classify_point(p) {
                Ok(Containment::In(_)) => assert!(inside, "block: In but oracle says out"),
                Ok(Containment::Out) => assert!(!inside, "block: Out but oracle says in"),
                Ok(Containment::On(_)) => {}
                Err(_) => {}
            }
            return;
        }
    };
    // Implicit sign oracle for sphere/torus.
    let val = surf.implicit(p);
    if val.abs() < 1e-5 {
        return; // tolerance band: any verdict acceptable
    }
    match body.classify_point(p) {
        Ok(Containment::In(_)) => assert!(val < 0.0, "In but implicit positive: {val}"),
        Ok(Containment::Out) => assert!(val > 0.0, "Out but implicit negative: {val}"),
        Ok(Containment::On(_)) => {}
        Err(_) => {
            // Ladder exhaustion is a permitted clean error; panics are not.
        }
    }
});
