//! Analytic surfaces and shared local differential geometry.

use crate::GeomError;
use keel_math::vec::Vec3;

/// Second-order local geometry of a parametric surface at (u, v):
/// derivatives, unit normal, fundamental forms, curvatures, and
/// principal directions. The contract M5 (Krawczyk tracing) and the
/// kernel/24 canonical-recovery service consume.
#[derive(Clone, Debug)]
pub struct SurfaceLocalGeometry {
    pub point: Vec3,
    pub du: Vec3,
    pub dv: Vec3,
    pub duu: Vec3,
    pub duv: Vec3,
    pub dvv: Vec3,
    /// Unit normal, du x dv normalized.
    pub normal: Vec3,
    /// First fundamental form E, F, G.
    pub e: f64,
    pub f: f64,
    pub g: f64,
    /// Second fundamental form L, M, N.
    pub l: f64,
    pub m: f64,
    pub n: f64,
    pub gaussian: f64,
    pub mean: f64,
    /// Principal curvatures, k1 >= k2.
    pub k1: f64,
    pub k2: f64,
    /// Unit principal directions in 3D for k1 and k2. At an umbilic
    /// (k1 == k2 to working precision) any orthonormal tangent pair is
    /// principal; we return normalized du and its in-plane
    /// perpendicular, deterministically.
    pub dir1: Vec3,
    pub dir2: Vec3,
}

/// Build local geometry from raw derivatives. Degenerate when the
/// normal vanishes (collapsed or singular parameterization).
pub(crate) fn local_geometry_from_ders(
    point: Vec3,
    du: Vec3,
    dv: Vec3,
    duu: Vec3,
    duv: Vec3,
    dvv: Vec3,
) -> Result<SurfaceLocalGeometry, GeomError> {
    let raw_n = du.cross(dv);
    let nn = raw_n.norm();
    let scale = du.norm().max(dv.norm());
    // Relative degeneracy test: |du x dv| tiny against the tangent
    // scale squared means the tangents are parallel or vanish. The
    // NaN-propagating comparison also rejects non-finite derivatives.
    if scale == 0.0 || nn.is_nan() || nn <= 1e-14 * scale * scale {
        return Err(GeomError::Degenerate);
    }
    let normal = raw_n * (1.0 / nn);
    let (e, f, g) = (du.dot(du), du.dot(dv), dv.dot(dv));
    let (l, m, n) = (duu.dot(normal), duv.dot(normal), dvv.dot(normal));
    let det1 = e * g - f * f; // == nn^2 > 0 here
    let gaussian = (l * n - m * m) / det1;
    let mean = (e * n - 2.0 * f * m + g * l) / (2.0 * det1);
    // Guard tiny negative discriminants from roundoff at umbilics.
    let disc = (mean * mean - gaussian).max(0.0).sqrt();
    let (k1, k2) = (mean + disc, mean - disc);
    let (dir1, dir2) = principal_dirs(k1, k2, e, f, g, l, m, n, du, dv, normal);
    Ok(SurfaceLocalGeometry {
        point,
        du,
        dv,
        duu,
        duv,
        dvv,
        normal,
        e,
        f,
        g,
        l,
        m,
        n,
        gaussian,
        mean,
        k1,
        k2,
        dir1,
        dir2,
    })
}

/// Principal directions: null vectors of (II - k I) in the {du, dv}
/// basis. Pick the larger row for stability; at an umbilic both rows
/// vanish and we fall back to the deterministic orthonormal pair.
#[allow(clippy::too_many_arguments)]
fn principal_dirs(
    k1: f64,
    k2: f64,
    e: f64,
    f: f64,
    g: f64,
    l: f64,
    m: f64,
    n: f64,
    du: Vec3,
    dv: Vec3,
    normal: Vec3,
) -> (Vec3, Vec3) {
    let tangent_dir = |k: f64| -> Option<Vec3> {
        let (r1a, r1b) = (l - k * e, m - k * f);
        let (r2a, r2b) = (m - k * f, n - k * g);
        let (a, b) = if r1a * r1a + r1b * r1b >= r2a * r2a + r2b * r2b {
            (r1b, -r1a)
        } else {
            (r2b, -r2a)
        };
        let d = du * a + dv * b;
        let dn = d.norm();
        // Reject when the row is numerically zero relative to the
        // form scale: umbilic.
        let row_scale = (e + g) * (1.0 + k.abs());
        if dn > 1e-10 * row_scale {
            Some(d * (1.0 / dn))
        } else {
            None
        }
    };
    match (tangent_dir(k1), tangent_dir(k2)) {
        (Some(d1), Some(d2)) => (d1, d2),
        (Some(d1), None) => (d1, normal.cross(d1)),
        (None, Some(d2)) => (d2.cross(normal), d2),
        (None, None) => {
            // Umbilic: deterministic orthonormal tangent pair.
            let d1 = du * (1.0 / du.norm());
            (d1, normal.cross(d1))
        }
    }
}
