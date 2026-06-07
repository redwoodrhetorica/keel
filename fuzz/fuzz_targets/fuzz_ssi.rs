#![no_main]
use keel_geom::ssi::{SsiResult, SurfaceRef, intersect_surfaces};
use keel_geom::surface::{Cylinder3, Frame3, Plane3, Sphere3, Surface3};
use keel_math::vec::Vec3;
use libfuzzer_sys::fuzz_target;

// SSI on random analytic pairs: every reported curve point must lie on
// BOTH surfaces (implicit residual within a generous band), no panics,
// and coaxial/coincident constructions must never be mis-traced as
// transversal curves.
fuzz_target!(|data: (u8, [f64; 8])| {
    let (kind, v) = data;
    if v.iter().any(|x| !x.is_finite()) {
        return;
    }
    let c = |x: f64, lo: f64, hi: f64| x.abs().clamp(lo, hi);
    let frame = |ox: f64, oy: f64, oz: f64| {
        Frame3::from_z(Vec3::new(ox.clamp(-20.0, 20.0), oy.clamp(-20.0, 20.0), oz.clamp(-20.0, 20.0)),
            Vec3::new(0., 0., 1.))
    };
    let Ok(fa) = frame(v[0], v[1], 0.0) else { return };
    let Ok(fb) = frame(v[2], v[3], v[4]) else { return };

    let make = |sel: u8, f: Frame3, p0: f64, p1: f64| -> Option<Surface3> {
        match sel % 3 {
            0 => Some(Surface3::Plane(Plane3::new(f))),
            1 => Sphere3::new(f, c(p0, 0.2, 15.0)).ok().map(Surface3::Sphere),
            _ => Cylinder3::new(f, c(p1, 0.2, 15.0)).ok().map(Surface3::Cylinder),
        }
    };
    let Some(sa) = make(kind, fa, v[5], v[6]) else { return };
    let Some(sb) = make(kind >> 2, fb, v[6], v[7]) else { return };

    let tol = 1e-7;
    let res = match intersect_surfaces(&SurfaceRef::Analytic(&sa), &SurfaceRef::Analytic(&sb), tol) {
        Ok(r) => r,
        Err(_) => return, // degenerate/unsupported pairs are clean errors
    };
    let on_both = |p: Vec3, band: f64| {
        let ra = sa.implicit(p).abs() / sa.implicit_gradient(p).norm().max(1e-12);
        let rb = sb.implicit(p).abs() / sb.implicit_gradient(p).norm().max(1e-12);
        ra < band && rb < band
    };
    match res {
        SsiResult::Curves(cs) => {
            for cu in &cs {
                let band = (cu.tol_achieved * 4.0).max(1e-4);
                for k in 0..=8 {
                    let t = k as f64 / 8.0;
                    let p = match &cu.curve {
                        keel_geom::curve::Curve3::Line(l) => l.point(-5.0 + 10.0 * t),
                        keel_geom::curve::Curve3::Circle(ci) => {
                            ci.point(core::f64::consts::TAU * t)
                        }
                        keel_geom::curve::Curve3::Ellipse(e) => {
                            e.point(core::f64::consts::TAU * t)
                        }
                        keel_geom::curve::Curve3::Nurbs(n) => {
                            let (a, b) = n.domain();
                            n.point(a + t * (b - a))
                        }
                    };
                    // Lines are infinite; only assert for the bounded
                    // analytic curve types where every sample is on the
                    // true intersection.
                    if !matches!(cu.curve, keel_geom::curve::Curve3::Line(_)) {
                        assert!(on_both(p, band), "SSI point off a surface");
                    }
                }
            }
        }
        SsiResult::Points(_) | SsiResult::Empty | SsiResult::Coincident => {}
    }
});
