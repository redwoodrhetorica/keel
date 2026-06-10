//! Surface from boundary curves, n-sided (parity item 68; dossier 26
//! sec 1.1: the bilinearly blended Coons patch, Coons 1967 MIT
//! MAC-TR-41 -- "the algebraic skeleton ... [used] directly by the
//! practical NURBS hole-fill"). MVP: the DISCRETE Coons interior on
//! the control net over four boundary control rows fit from sampled
//! sides (end-pinned LSQ, so corners interpolate exactly). N != 4
//! sides are joined and re-quartered by arc length, which places the
//! extra corners INSIDE the virtual sides where the cubic fit rounds
//! them -- the certificate reports that honestly; the corner-exact
//! central-split n-sided scheme (dossier 26 sec 4) and the variational
//! fairing upgrade (dossiers 31/32; patent-clear per LOG Addendum 131)
//! are follow-ups on this same skeleton. The boundary is CERTIFIED
//! against fresh samples of the input sides, foreign-fit style: when
//! the certificate misses `tol`, callers see it in `tol_achieved` and
//! decline.

use crate::GeomError;
use crate::fit::lsq_fit;
use crate::nurbs_surface::NurbsSurface;
use keel_math::vec::Vec3;

/// A certified boundary fill.
#[derive(Clone, Debug)]
pub struct BoundaryFill {
    pub surface: NurbsSurface,
    /// Max boundary deviation against fresh side samples, times the
    /// safety factor.
    pub tol_achieved: f64,
}

/// Same between-sample inflation as the other certified fits.
const SAFETY: f64 = 2.0;
const ESCALATION: [usize; 4] = [4, 8, 16, 32];

/// Fill a closed chain of boundary sides (each a polyline of 2+ finite
/// points, tip-to-tail around the loop) with one NURBS patch within
/// `tol` if possible. The best fill is returned even when `tol` is not
/// met (callers read `tol_achieved`; `Body::filled_sheet` declines).
pub fn fill_boundary(sides: &[Vec<Vec3>], tol: f64) -> Result<BoundaryFill, GeomError> {
    if tol <= 0.0 || !tol.is_finite() {
        return Err(GeomError::Degenerate);
    }
    if sides.len() < 3 {
        return Err(GeomError::Degenerate);
    }
    let chain_tol = tol.max(1e-9);
    for (k, s) in sides.iter().enumerate() {
        if s.len() < 2 || s.iter().any(|p| !p.is_finite()) {
            return Err(GeomError::Degenerate);
        }
        let next = &sides[(k + 1) % sides.len()];
        if (s[s.len() - 1] - next[0]).norm() > chain_tol {
            return Err(GeomError::Degenerate);
        }
    }
    // The four square sides: the given sides directly for N == 4, else
    // the joined closed polyline re-quartered by arc length.
    let quad: [Vec<Vec3>; 4] = if sides.len() == 4 {
        [
            sides[0].clone(),
            sides[1].clone(),
            sides[2].clone(),
            sides[3].clone(),
        ]
    } else {
        let mut loop_pts: Vec<Vec3> = Vec::new();
        for s in sides {
            // Skip each side's first point after the first side (it
            // coincides with the previous side's last point).
            let skip = usize::from(!loop_pts.is_empty());
            loop_pts.extend(s.iter().skip(skip));
        }
        let total = polyline_length(&loop_pts);
        if total <= 0.0 {
            return Err(GeomError::Degenerate);
        }
        let q = total / 4.0;
        [
            polyline_slice(&loop_pts, 0.0, q),
            polyline_slice(&loop_pts, q, 2.0 * q),
            polyline_slice(&loop_pts, 2.0 * q, 3.0 * q),
            polyline_slice(&loop_pts, 3.0 * q, total),
        ]
    };

    let mut best: Option<BoundaryFill> = None;
    for &n in &ESCALATION {
        let m = 2 * n + 1;
        let params: Vec<f64> = (0..m).map(|i| i as f64 / (m - 1) as f64).collect();
        // Resample each square side to m points by arc length so all
        // four LSQ fits share the parameterization (hence knots).
        let s0 = resample(&quad[0], m); // v=0, u: 0 -> 1
        let s1 = resample(&quad[1], m); // u=1, v: 0 -> 1
        let mut s2 = resample(&quad[2], m); // C -> D; reverse for u: 0 -> 1
        s2.reverse();
        let mut s3 = resample(&quad[3], m); // D -> A; reverse for v: 0 -> 1
        s3.reverse();
        let (Ok(c0), Ok(c1), Ok(d0), Ok(d1)) = (
            lsq_fit(&s0, &params, n),
            lsq_fit(&s2, &params, n),
            lsq_fit(&s3, &params, n),
            lsq_fit(&s1, &params, n),
        ) else {
            continue;
        };
        let (r0, r1) = (c0.control_points(), c1.control_points()); // rows j=0, j=n-1
        let (q0, q1) = (d0.control_points(), d1.control_points()); // cols i=0, i=n-1
        if r0.len() != n || r1.len() != n || q0.len() != n || q1.len() != n {
            continue;
        }
        // Assemble the net: boundary rows/columns pinned (corners from
        // the row fits; the column fits' ends agree within chain_tol),
        // interior by the discrete bilinearly blended Coons sum.
        let (ca, cb, cc, cd) = (r0[0], r0[n - 1], r1[n - 1], r1[0]);
        let mut net = vec![Vec3::ZERO; n * n];
        for i in 0..n {
            net[i * n] = r0[i];
            net[i * n + (n - 1)] = r1[i];
        }
        for j in 1..(n - 1) {
            net[j] = q0[j];
            net[(n - 1) * n + j] = q1[j];
        }
        for i in 1..(n - 1) {
            let s = i as f64 / (n - 1) as f64;
            for j in 1..(n - 1) {
                let t = j as f64 / (n - 1) as f64;
                let lofts = net[i * n] * (1.0 - t)
                    + net[i * n + (n - 1)] * t
                    + net[j] * (1.0 - s)
                    + net[(n - 1) * n + j] * s;
                let corners = ca * ((1.0 - s) * (1.0 - t))
                    + cb * (s * (1.0 - t))
                    + cc * (s * t)
                    + cd * ((1.0 - s) * t);
                net[i * n + j] = lofts - corners;
            }
        }
        let kv_u = c0.knot_vector();
        let kv_v = d0.knot_vector();
        let Ok(surface) = NurbsSurface::new(
            kv_u.degree(),
            kv_v.degree(),
            kv_u.knots().to_vec(),
            kv_v.knots().to_vec(),
            net,
            None,
        ) else {
            continue;
        };
        // Certify the boundary against fresh side samples (between the
        // fit samples: 2m+1 per side).
        let dense = 2 * m + 1;
        let dev = {
            type SideEval<'a> = (&'a [Vec3], Box<dyn Fn(f64) -> Vec3 + 'a>);
            let side_eval: [SideEval<'_>; 4] = [
                (&s0, Box::new(|s| surface.point(s, 0.0))),
                (&s1, Box::new(|s| surface.point(1.0, s))),
                (&s2, Box::new(|s| surface.point(s, 1.0))),
                (&s3, Box::new(|s| surface.point(0.0, s))),
            ];
            let mut dev = 0.0f64;
            for (pts, eval) in &side_eval {
                let dense_pts = resample(pts, dense);
                for (k, q) in dense_pts.iter().enumerate() {
                    let s = k as f64 / (dense - 1) as f64;
                    dev = dev.max((eval(s) - *q).norm());
                }
            }
            dev
        };
        let tol_achieved = dev * SAFETY;
        if best.as_ref().is_none_or(|b| tol_achieved < b.tol_achieved) {
            best = Some(BoundaryFill {
                surface,
                tol_achieved,
            });
        }
        if best.as_ref().is_some_and(|b| b.tol_achieved <= tol) {
            break;
        }
    }
    best.ok_or(GeomError::Degenerate)
}

fn polyline_length(pts: &[Vec3]) -> f64 {
    pts.windows(2).map(|w| (w[1] - w[0]).norm()).sum()
}

/// The sub-polyline between arc lengths a and b (split points
/// interpolated on segments).
fn polyline_slice(pts: &[Vec3], a: f64, b: f64) -> Vec<Vec3> {
    let mut out = vec![point_at(pts, a)];
    let mut acc = 0.0;
    for w in pts.windows(2) {
        let seg = (w[1] - w[0]).norm();
        let end = acc + seg;
        if end > a && end < b {
            out.push(w[1]);
        }
        acc = end;
    }
    out.push(point_at(pts, b));
    out
}

/// Point at arc length `s` along the polyline.
fn point_at(pts: &[Vec3], s: f64) -> Vec3 {
    let mut acc = 0.0;
    for w in pts.windows(2) {
        let seg = (w[1] - w[0]).norm();
        if acc + seg >= s && seg > 0.0 {
            let f = ((s - acc) / seg).clamp(0.0, 1.0);
            return w[0] + (w[1] - w[0]) * f;
        }
        acc += seg;
    }
    pts[pts.len() - 1]
}

/// Resample a polyline to `m` points uniformly by arc length
/// (endpoints preserved exactly).
fn resample(pts: &[Vec3], m: usize) -> Vec<Vec3> {
    let total = polyline_length(pts);
    if total <= 0.0 {
        return vec![pts[0]; m];
    }
    let mut out = Vec::with_capacity(m);
    out.push(pts[0]);
    for k in 1..(m - 1) {
        out.push(point_at(pts, total * k as f64 / (m - 1) as f64));
    }
    out.push(pts[pts.len() - 1]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn straight(a: Vec3, b: Vec3, n: usize) -> Vec<Vec3> {
        (0..n)
            .map(|i| a + (b - a) * (i as f64 / (n - 1) as f64))
            .collect()
    }

    #[test]
    fn flat_square_fills_to_a_plane() {
        let p = |x: f64, y: f64| Vec3::new(x, y, 0.0);
        let sides = vec![
            straight(p(0., 0.), p(2., 0.), 5),
            straight(p(2., 0.), p(2., 2.), 5),
            straight(p(2., 2.), p(0., 2.), 5),
            straight(p(0., 2.), p(0., 0.), 5),
        ];
        let fill = fill_boundary(&sides, 1e-9).unwrap();
        assert!(
            fill.tol_achieved <= 1e-9,
            "flat boundary must certify (got {})",
            fill.tol_achieved
        );
        // The whole patch is flat.
        for i in 0..=8 {
            for j in 0..=8 {
                let q = fill.surface.point(i as f64 / 8.0, j as f64 / 8.0);
                assert!(q.z.abs() < 1e-12, "interior off-plane: {}", q.z);
            }
        }
    }

    #[test]
    fn saddle_boundary_certifies() {
        // One corner lifted: a genuinely doubly-curved fill whose
        // boundary must still be reproduced within the certificate.
        let p = |x: f64, y: f64, z: f64| Vec3::new(x, y, z);
        let sides = vec![
            straight(p(0., 0., 0.), p(2., 0., 0.), 9),
            straight(p(2., 0., 0.), p(2., 2., 1.), 9),
            straight(p(2., 2., 1.), p(0., 2., 0.), 9),
            straight(p(0., 2., 0.), p(0., 0., 0.), 9),
        ];
        let fill = fill_boundary(&sides, 1e-6).unwrap();
        assert!(
            fill.tol_achieved <= 1e-6,
            "saddle boundary must certify (got {})",
            fill.tol_achieved
        );
        // Corners interpolate exactly (end-pinned fits).
        assert!((fill.surface.point(1.0, 1.0) - p(2., 2., 1.)).norm() < 1e-12);
    }

    #[test]
    fn hexagon_quarters_and_fills() {
        // Six planar sides -> the arc-length quartering path; the fill
        // stays in the plane and the boundary certifies.
        let c = |a: f64| Vec3::new(a.cos(), a.sin(), 0.0);
        let tau = core::f64::consts::TAU;
        let sides: Vec<Vec<Vec3>> = (0..6)
            .map(|k| straight(c(tau * k as f64 / 6.0), c(tau * (k + 1) as f64 / 6.0), 7))
            .collect();
        // Quartering puts hexagon corners INSIDE the virtual sides; the
        // cubic boundary fit rounds them, and the certificate honestly
        // reports it (~7e-3 here). The dossier-26 central-split scheme
        // is the corner-exact n-sided follow-up.
        let fill = fill_boundary(&sides, 2e-2).unwrap();
        assert!(
            fill.tol_achieved <= 2e-2,
            "hexagon boundary must certify loosely (got {})",
            fill.tol_achieved
        );
        for i in 0..=6 {
            for j in 0..=6 {
                let q = fill.surface.point(i as f64 / 6.0, j as f64 / 6.0);
                assert!(q.z.abs() < 1e-9, "hexagon fill off-plane: {}", q.z);
            }
        }
    }

    #[test]
    fn open_chain_is_rejected() {
        let p = |x: f64, y: f64| Vec3::new(x, y, 0.0);
        let sides = vec![
            straight(p(0., 0.), p(2., 0.), 3),
            straight(p(2., 0.), p(2., 2.), 3),
            // Gap: next side starts elsewhere.
            straight(p(3., 2.), p(0., 2.), 3),
            straight(p(0., 2.), p(0., 0.), 3),
        ];
        assert!(fill_boundary(&sides, 1e-6).is_err());
    }
}
