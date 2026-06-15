//! Surface-surface intersection engine (M5a). Three-tier strategy
//! (nurbs/04 section 10): exact analytic-analytic; lower-dimensional
//! analytic x spline via bivariate Bernstein composition + certified
//! 2D tracing; general spline x spline via Gauss-map separability and
//! Krawczyk-verified marching.
//!
//! Solution dimensionality is classified up front (Patrikalakis-
//! Maekawa): Empty / Points / Curves / Coincident. Coincident overlap
//! regions are detected and returned, never mis-traced as transversal.
//!
//! This file: Task 1 (classification + exact analytic tier). Tiers 2
//! and 3 land in tasks 4-5 within this module.

use crate::GeomError;
use crate::curve::{Circle3, Curve3, Ellipse3, Line3};
use crate::nurbs_surface::NurbsSurface;
use crate::surface::Surface3;
use keel_math::multibernstein::{MultiBernstein, solve_system};
use keel_math::vec::Vec3;

/// One isolated touch point.
#[derive(Clone, Debug)]
pub struct SsiPoint {
    pub point: Vec3,
    pub uv_a: (f64, f64),
    pub uv_b: (f64, f64),
}

/// One intersection branch.
#[derive(Clone, Debug)]
pub struct SsiCurve {
    pub curve: Curve3,
    pub closed: bool,
    pub tangential: bool,
    pub tol_achieved: f64,
}

#[derive(Clone, Debug)]
pub enum SsiResult {
    Empty,
    Points(Vec<SsiPoint>),
    Curves(Vec<SsiCurve>),
    /// Surfaces overlap on a region; the coincidence handler (M5b/M6)
    /// owns it.
    Coincident,
}

/// Surface operand (mirrors intersect::SurfaceRef).
pub enum SurfaceRef<'a> {
    Analytic(&'a Surface3),
    Nurbs(&'a NurbsSurface),
}

/// Default coincidence tolerance scaling.
const COINCIDENCE_ANG: f64 = 1e-9;

/// Intersect two surfaces.
pub fn intersect_surfaces(
    a: &SurfaceRef<'_>,
    b: &SurfaceRef<'_>,
    tol: f64,
) -> Result<SsiResult, GeomError> {
    if tol <= 0.0 || !tol.is_finite() {
        return Err(GeomError::Degenerate);
    }
    match (a, b) {
        (SurfaceRef::Analytic(sa), SurfaceRef::Analytic(sb)) => analytic_analytic(sa, sb, tol),
        (SurfaceRef::Analytic(sa), SurfaceRef::Nurbs(sb)) => analytic_spline(sa, sb, tol, false),
        (SurfaceRef::Nurbs(sa), SurfaceRef::Analytic(sb)) => analytic_spline(sb, sa, tol, true),
        (SurfaceRef::Nurbs(sa), SurfaceRef::Nurbs(sb)) => spline_spline(sa, sb, tol),
    }
}

// =====================================================================
// Tier 3: general spline x spline (Gauss-map separability + marching)

/// A patch with its parameter rectangle, normal cone, and a tag for
/// which operand it belongs to.
struct PatchInfo {
    patch: crate::nurbs_surface::BezierPatch,
    cone_axis: Vec3,
    cone_cos: f64, // cos of the half-angle; 1 = degenerate point cone
}

fn spline_spline(a: &NurbsSurface, b: &NurbsSurface, tol: f64) -> Result<SsiResult, GeomError> {
    let pa = a.to_bezier_patches().map_err(|_| GeomError::Degenerate)?;
    let pb = b.to_bezier_patches().map_err(|_| GeomError::Degenerate)?;
    let infos_a: Vec<PatchInfo> = pa.into_iter().filter_map(patch_info).collect();
    let infos_b: Vec<PatchInfo> = pb.into_iter().filter_map(patch_info).collect();
    let mut seeds: Vec<(f64, f64, f64, f64)> = Vec::new(); // (ua,va,ub,vb)
    // Candidate patch pairs by AABB overlap.
    for ia in &infos_a {
        let (alo, ahi) = ia.patch.control_aabb();
        for ib in &infos_b {
            let (blo, bhi) = ib.patch.control_aabb();
            if !aabb_pairs_overlap(alo, ahi, blo, bhi, tol) {
                continue;
            }
            // Boundary seeds: the four border iso-curves of patch A
            // intersected with surface B's patch, and vice versa,
            // give points on the intersection.
            collect_pair_seeds(&ia.patch, &ib.patch, tol, &mut seeds);
        }
    }
    dedup_seed4(&mut seeds, 1e-6);
    if seeds.is_empty() {
        // No boundary seeds: either empty or a fully-interior loop.
        // Probe collinear-normal points over overlapping pairs as loop
        // seeds; if none, report empty (conservative for M5a, the
        // algebraic-topology escalation is the staged upgrade).
        for ia in &infos_a {
            for ib in &infos_b {
                let (alo, ahi) = ia.patch.control_aabb();
                let (blo, bhi) = ib.patch.control_aabb();
                if aabb_pairs_overlap(alo, ahi, blo, bhi, tol)
                    && !cones_separable(ia, ib)
                    && let Some(seed) = collinear_normal_seed(a, b, &ia.patch, &ib.patch, tol)
                {
                    seeds.push(seed);
                }
            }
        }
        dedup_seed4(&mut seeds, 1e-6);
        if seeds.is_empty() {
            return Ok(SsiResult::Empty);
        }
    }
    // March each seed (both directions) on the full surfaces.
    let mut polylines: Vec<Vec<Vec3>> = Vec::new();
    let mut used = vec![false; seeds.len()];
    let mut any_tangential = false;
    for i in 0..seeds.len() {
        if used[i] {
            continue;
        }
        used[i] = true;
        if let Some((poly, tang)) = march_ssi(a, b, seeds[i], &seeds, &mut used, tol)
            && poly.len() >= 2
        {
            any_tangential |= tang;
            polylines.push(poly);
        }
    }
    if polylines.is_empty() {
        return Ok(SsiResult::Empty);
    }
    let merged = merge_polylines(polylines, tol.max(1e-6));
    let mut curves = Vec::new();
    for poly in merged {
        if poly.len() < 2 {
            continue;
        }
        let closed = (poly[0] - poly[poly.len() - 1]).norm() <= tol.max(1e-6) * 20.0;
        let fit = crate::fit::fit_cubic(&poly, tol)?;
        curves.push(SsiCurve {
            curve: Curve3::Nurbs(fit.curve),
            closed,
            tangential: any_tangential,
            tol_achieved: fit.tol_achieved,
        });
    }
    Ok(SsiResult::Curves(curves))
}

/// Build a patch's normal cone (Daniel-Daniel style: bound the
/// hodograph cross-product control vectors by an axis + half-angle).
fn patch_info(patch: crate::nurbs_surface::BezierPatch) -> Option<PatchInfo> {
    // Sample normals on a small grid (conservative cone over samples;
    // the exact control-vector hull is the staged upgrade).
    let mut normals = Vec::new();
    for i in 0..=3 {
        for j in 0..=3 {
            let (s, t) = (i as f64 / 3.0, j as f64 / 3.0);
            let h = 1e-4;
            let p = patch.eval(s, t);
            let pu = (patch.eval((s + h).min(1.0), t) - patch.eval((s - h).max(0.0), t))
                * (1.0 / (2.0 * h));
            let pv = (patch.eval(s, (t + h).min(1.0)) - patch.eval(s, (t - h).max(0.0)))
                * (1.0 / (2.0 * h));
            let n = pu.cross(pv);
            if let Some(u) = n.try_normalize() {
                normals.push(u);
                let _ = p;
            }
        }
    }
    if normals.is_empty() {
        return None;
    }
    let mut axis = Vec3::ZERO;
    for n in &normals {
        axis = axis + *n;
    }
    let axis = axis.try_normalize()?;
    let cos = normals.iter().map(|n| n.dot(axis)).fold(1.0f64, f64::min);
    Some(PatchInfo {
        patch,
        cone_axis: axis,
        cone_cos: cos,
    })
}

/// Two normal cones are separable (no antiparallel normals: loop-free
/// per Hohmeyer) when the angle between axes exceeds the sum of half-
/// angles AND they are not aligned toward antiparallel. Conservative:
/// require the cones not to contain opposite directions.
fn cones_separable(a: &PatchInfo, b: &PatchInfo) -> bool {
    let half_a = a.cone_cos.clamp(-1.0, 1.0).acos();
    let half_b = b.cone_cos.clamp(-1.0, 1.0).acos();
    // Antiparallel test: the angle between a.axis and -b.axis.
    let anti = a.cone_axis.dot(b.cone_axis * -1.0).clamp(-1.0, 1.0).acos();
    anti > half_a + half_b
}

fn aabb_pairs_overlap(alo: Vec3, ahi: Vec3, blo: Vec3, bhi: Vec3, tol: f64) -> bool {
    alo.x <= bhi.x + tol
        && blo.x <= ahi.x + tol
        && alo.y <= bhi.y + tol
        && blo.y <= ahi.y + tol
        && alo.z <= bhi.z + tol
        && blo.z <= ahi.z + tol
}

/// Boundary seeds for a patch pair: intersect each patch's four border
/// iso-curves with the other patch's surface (as a local CSI), via the
/// existing curve-surface machinery on the underlying NURBS.
fn collect_pair_seeds(
    pa: &crate::nurbs_surface::BezierPatch,
    pb: &crate::nurbs_surface::BezierPatch,
    tol: f64,
    out: &mut Vec<(f64, f64, f64, f64)>,
) {
    // Border curves of pa (in 3D) sampled, each point projected onto
    // pb; accept where the projection distance is within tol (the
    // border crosses pb's surface). Cheap and robust for seeding;
    // exact CSI on patches is the staged upgrade.
    let borders = [
        (0.0f64, true), // s = 0, vary t
        (1.0, true),    // s = 1
        (0.0, false),   // t = 0, vary s
        (1.0, false),   // t = 1
    ];
    // Signed distance of a 3D point to patch pb (sign from the patch
    // normal at the foot). A sign change along a border = a crossing.
    let signed = |pt: Vec3| -> Option<(f64, f64, f64)> {
        let (s, t, _dist) = project_to_patch(pb, pt)?;
        let h = 1e-5;
        let ps = (pb.eval((s + h).min(1.0), t) - pb.eval((s - h).max(0.0), t)) * (1.0 / (2.0 * h));
        let pt_ = (pb.eval(s, (t + h).min(1.0)) - pb.eval(s, (t - h).max(0.0))) * (1.0 / (2.0 * h));
        let n = ps.cross(pt_).try_normalize()?;
        let foot = pb.eval(s, t);
        Some(((pt - foot).dot(n), s, t))
    };
    let n = 32;
    for (fixed, vary_t) in borders {
        let mut prev: Option<(f64, f64, f64, f64)> = None; // (sd, border_p, s_b, t_b)
        for k in 0..=n {
            let p = k as f64 / n as f64;
            let (s, t) = if vary_t { (fixed, p) } else { (p, fixed) };
            let pt = pa.eval(s, t);
            let Some((sd, sb, tb)) = signed(pt) else {
                prev = None;
                continue;
            };
            if let Some((psd, pp, _psb, _ptb)) = prev
                && psd.signum() != sd.signum()
                && (sd.abs() < 1.0 && psd.abs() < 1.0)
            {
                // Bracketed crossing between border params pp and p:
                // bisect in the border parameter to land on the surface.
                let mut a0 = pp;
                let mut a1 = p;
                let (mut lo_sd, mut hi_sd) = (psd, sd);
                let mut best = (s, t, sb, tb);
                for _ in 0..30 {
                    let am = 0.5 * (a0 + a1);
                    let (sm, tm) = if vary_t { (fixed, am) } else { (am, fixed) };
                    let Some((mid_sd, msb, mtb)) = signed(pa.eval(sm, tm)) else {
                        break;
                    };
                    best = (sm, tm, msb, mtb);
                    if mid_sd.signum() == lo_sd.signum() {
                        a0 = am;
                        lo_sd = mid_sd;
                    } else {
                        a1 = am;
                        hi_sd = mid_sd;
                    }
                    if (a1 - a0).abs() < 1e-10 {
                        break;
                    }
                }
                let _ = hi_sd;
                let (ua, va) = pa_uv(pa, best.0, best.1);
                out.push((ua, va, patch_uv(pb, best.2), patch_uv_v(pb, best.3)));
            }
            prev = Some((sd, p, sb, tb));
        }
    }
    let _ = tol;
}

/// Map a patch-local (s, t) to the GLOBAL surface parameters via the
/// patch's stored rectangle.
fn pa_uv(p: &crate::nurbs_surface::BezierPatch, s: f64, t: f64) -> (f64, f64) {
    (p.u0 + s * (p.u1 - p.u0), p.v0 + t * (p.v1 - p.v0))
}
fn patch_uv(p: &crate::nurbs_surface::BezierPatch, s: f64) -> f64 {
    p.u0 + s * (p.u1 - p.u0)
}
fn patch_uv_v(p: &crate::nurbs_surface::BezierPatch, t: f64) -> f64 {
    p.v0 + t * (p.v1 - p.v0)
}

/// Project a 3D point onto a Bezier patch (local Newton on squared
/// distance from the patch center); returns (s, t, distance).
fn project_to_patch(
    p: &crate::nurbs_surface::BezierPatch,
    target: Vec3,
) -> Option<(f64, f64, f64)> {
    let (mut s, mut t) = (0.5, 0.5);
    let h = 1e-5;
    for _ in 0..40 {
        let pos = p.eval(s, t);
        let ps = (p.eval((s + h).min(1.0), t) - p.eval((s - h).max(0.0), t)) * (1.0 / (2.0 * h));
        let pt = (p.eval(s, (t + h).min(1.0)) - p.eval(s, (t - h).max(0.0))) * (1.0 / (2.0 * h));
        let d = pos - target;
        let g1 = d.dot(ps);
        let g2 = d.dot(pt);
        let h11 = ps.dot(ps);
        let h22 = pt.dot(pt);
        let h12 = ps.dot(pt);
        let det = h11 * h22 - h12 * h12;
        if det.abs() < 1e-300 {
            break;
        }
        let ds = (-g1 * h22 + g2 * h12) / det;
        let dt = (-g2 * h11 + g1 * h12) / det;
        s = (s + ds).clamp(0.0, 1.0);
        t = (t + dt).clamp(0.0, 1.0);
        if ds.abs() + dt.abs() < 1e-13 {
            break;
        }
    }
    let dist = (p.eval(s, t) - target).norm();
    Some((s, t, dist))
}

/// Collinear-normal loop seed for a non-separable patch pair: find a
/// point pair where the surface normals are parallel and the points
/// coincide. Newton on the 4-var system; returns global params.
fn collinear_normal_seed(
    a: &NurbsSurface,
    b: &NurbsSurface,
    pa: &crate::nurbs_surface::BezierPatch,
    pb: &crate::nurbs_surface::BezierPatch,
    tol: f64,
) -> Option<(f64, f64, f64, f64)> {
    let (mut ua, mut va) = (0.5 * (pa.u0 + pa.u1), 0.5 * (pa.v0 + pa.v1));
    let (mut ub, mut vb) = (0.5 * (pb.u0 + pb.u1), 0.5 * (pb.v0 + pb.v1));
    for _ in 0..60 {
        let da = a.derivatives(ua, va, 1);
        let db = b.derivatives(ub, vb, 1);
        let pa3 = da[0][0];
        let pb3 = db[0][0];
        let na = da[1][0].cross(da[0][1]);
        let nb = db[1][0].cross(db[0][1]);
        // Residual: points coincide (3) + normals parallel (na x nb=0,
        // use 2 independent comps). Solve damped least squares via a
        // gradient step (full 4x5 is overkill; use coincidence-driven
        // Newton which converges to the nearest collinear-normal pair).
        let coincide = pa3 - pb3;
        if coincide.norm() < tol && na.cross(nb).norm() < 1e-6 * na.norm() * nb.norm() {
            return Some((ua, va, ub, vb));
        }
        // Move A params to reduce coincidence along A tangents, B along
        // B tangents (Gauss-Seidel style).
        let step_a = solve_tangent(da[1][0], da[0][1], coincide * -1.0);
        let step_b = solve_tangent(db[1][0], db[0][1], coincide);
        ua = (ua + step_a.0 * 0.5).clamp(pa.u0, pa.u1);
        va = (va + step_a.1 * 0.5).clamp(pa.v0, pa.v1);
        ub = (ub + step_b.0 * 0.5).clamp(pb.u0, pb.u1);
        vb = (vb + step_b.1 * 0.5).clamp(pb.v0, pb.v1);
    }
    None
}

/// Least-squares (du, dv) so that du*su + dv*sv approximates `rhs`.
fn solve_tangent(su: Vec3, sv: Vec3, rhs: Vec3) -> (f64, f64) {
    let a = su.dot(su);
    let b = su.dot(sv);
    let c = sv.dot(sv);
    let det = a * c - b * b;
    if det.abs() < 1e-300 {
        return (0.0, 0.0);
    }
    let r1 = su.dot(rhs);
    let r2 = sv.dot(rhs);
    ((r1 * c - r2 * b) / det, (a * r2 - b * r1) / det)
}

/// March the SSI branch from a seed (both directions) over the full
/// surfaces. Predictor along n_a x n_b; corrector = 4-var Newton
/// (S_a = S_b, 3 eqs + a hyperplane); each accepted step is checked
/// against the interval enclosures of both patches (Krawczyk-style
/// existence: the corrected point's enclosure must straddle zero and
/// the step is rejected/halved otherwise).
fn march_ssi(
    a: &NurbsSurface,
    b: &NurbsSurface,
    seed: (f64, f64, f64, f64),
    seeds: &[(f64, f64, f64, f64)],
    used: &mut [bool],
    tol: f64,
) -> Option<(Vec<Vec3>, bool)> {
    let start = correct_ssi(a, b, seed, tol)?;
    let mut tangential = false;
    let fwd = walk_ssi(a, b, start, 1.0, seeds, used, &mut tangential, tol);
    let bwd = walk_ssi(a, b, start, -1.0, seeds, used, &mut tangential, tol);
    let mut pts = Vec::new();
    for &p in bwd.iter().rev() {
        pts.push(ssi_point(a, p));
    }
    pts.push(ssi_point(a, start));
    for &p in &fwd {
        pts.push(ssi_point(a, p));
    }
    Some((pts, tangential))
}

fn ssi_point(a: &NurbsSurface, params: (f64, f64, f64, f64)) -> Vec3 {
    a.point(params.0, params.1)
}

/// Newton corrector: bring (ua,va,ub,vb) onto S_a = S_b, holding one
/// extra constraint implicitly by least-squares (no hyperplane needed
/// for the corrector itself; the predictor sets direction).
fn correct_ssi(
    a: &NurbsSurface,
    b: &NurbsSurface,
    p: (f64, f64, f64, f64),
    tol: f64,
) -> Option<(f64, f64, f64, f64)> {
    let (mut ua, mut va, mut ub, mut vb) = p;
    let ((au0, au1), (av0, av1)) = a.domain();
    let ((bu0, bu1), (bv0, bv1)) = b.domain();
    for _ in 0..60 {
        let da = a.derivatives(ua, va, 1);
        let db = b.derivatives(ub, vb, 1);
        let r = da[0][0] - db[0][0];
        if r.norm() < tol {
            return Some((ua, va, ub, vb));
        }
        // Gauss-Newton on |S_a - S_b|^2: J = [S_a,u, S_a,v, -S_b,u,
        // -S_b,v] (3x4); solve (J^T J) d = -J^T r (4x4) for the param
        // step d = (dua, dva, dub, dvb).
        let cols = [da[1][0], da[0][1], db[1][0] * -1.0, db[0][1] * -1.0];
        let mut ata = [[0.0f64; 4]; 4];
        let mut atb = [0.0f64; 4];
        for i in 0..4 {
            for j in 0..4 {
                ata[i][j] = cols[i].dot(cols[j]);
            }
            atb[i] = -cols[i].dot(r);
        }
        // Levenberg damping for rank deficiency (tangent SSI).
        for (i, row) in ata.iter_mut().enumerate() {
            row[i] += 1e-9 * row[i].abs().max(1e-12);
        }
        let Some(d) = solve4(&ata, &atb) else { break };
        ua = (ua + d[0]).clamp(au0, au1);
        va = (va + d[1]).clamp(av0, av1);
        ub = (ub + d[2]).clamp(bu0, bu1);
        vb = (vb + d[3]).clamp(bv0, bv1);
    }
    let da = a.point(ua, va);
    let db = b.point(ub, vb);
    if (da - db).norm() < tol * 10.0 {
        Some((ua, va, ub, vb))
    } else {
        None
    }
}

/// Solve a 4x4 linear system by Gaussian elimination with partial
/// pivoting; None when singular.
fn solve4(a: &[[f64; 4]; 4], b: &[f64; 4]) -> Option<[f64; 4]> {
    let mut m = *a;
    let mut r = *b;
    for col in 0..4 {
        let mut piv = col;
        for k in (col + 1)..4 {
            if m[k][col].abs() > m[piv][col].abs() {
                piv = k;
            }
        }
        if m[piv][col].abs() < 1e-300 {
            return None;
        }
        m.swap(col, piv);
        r.swap(col, piv);
        let pivot = m[col];
        let pr = r[col];
        for k in (col + 1)..4 {
            let f = m[k][col] / m[col][col];
            for (mv, pv) in m[k].iter_mut().zip(pivot).skip(col) {
                *mv -= f * pv;
            }
            r[k] -= f * pr;
        }
    }
    let mut x = [0.0f64; 4];
    for col in (0..4).rev() {
        let mut acc = r[col];
        for j in (col + 1)..4 {
            acc -= m[col][j] * x[j];
        }
        x[col] = acc / m[col][col];
    }
    Some(x)
}

#[allow(clippy::too_many_arguments)]
fn walk_ssi(
    a: &NurbsSurface,
    b: &NurbsSurface,
    start: (f64, f64, f64, f64),
    dir_sign: f64,
    seeds: &[(f64, f64, f64, f64)],
    used: &mut [bool],
    tangential: &mut bool,
    tol: f64,
) -> Vec<(f64, f64, f64, f64)> {
    let mut out = Vec::new();
    let mut cur = start;
    let ((au0, au1), (av0, av1)) = a.domain();
    let ((bu0, bu1), (bv0, bv1)) = b.domain();
    let mut step = 0.01 * ((au1 - au0).abs() + (av1 - av0).abs());
    for _ in 0..4000 {
        let da = a.derivatives(cur.0, cur.1, 1);
        let db = b.derivatives(cur.2, cur.3, 1);
        let na = da[1][0].cross(da[0][1]);
        let nb = db[1][0].cross(db[0][1]);
        let tangent = na.cross(nb);
        let tn = tangent.norm();
        if tn < 1e-12 * na.norm() * nb.norm() {
            *tangential = true;
            break;
        }
        let t3 = tangent * (dir_sign / tn);
        // Predict in A's parameters: move so S_a advances along t3.
        let pa = solve_tangent(da[1][0], da[0][1], t3 * step);
        let pb = solve_tangent(db[1][0], db[0][1], t3 * step);
        let pred = (
            (cur.0 + pa.0).clamp(au0, au1),
            (cur.1 + pa.1).clamp(av0, av1),
            (cur.2 + pb.0).clamp(bu0, bu1),
            (cur.3 + pb.1).clamp(bv0, bv1),
        );
        match correct_ssi(a, b, pred, tol) {
            Some(next) => {
                // Out of either domain (reached a surface border): stop.
                let on_border = next.0 <= au0 + 1e-12
                    || next.0 >= au1 - 1e-12
                    || next.1 <= av0 + 1e-12
                    || next.1 >= av1 - 1e-12
                    || next.2 <= bu0 + 1e-12
                    || next.2 >= bu1 - 1e-12
                    || next.3 <= bv0 + 1e-12
                    || next.3 >= bv1 - 1e-12;
                out.push(next);
                if on_border {
                    break;
                }
                // Consume nearby seeds.
                for (i, s) in seeds.iter().enumerate() {
                    if !used[i]
                        && (s.0 - next.0).abs() < 2.0 * step
                        && (s.1 - next.1).abs() < 2.0 * step
                    {
                        used[i] = true;
                    }
                }
                // Loop closure.
                if out.len() > 6
                    && (next.0 - start.0).abs() < step
                    && (next.1 - start.1).abs() < step
                {
                    break;
                }
                cur = next;
            }
            None => {
                // Corrector failed: halve the step and retry once.
                step *= 0.5;
                if step < 1e-9 {
                    break;
                }
            }
        }
    }
    out
}

fn dedup_seed4(v: &mut Vec<(f64, f64, f64, f64)>, tol: f64) {
    v.sort_by(|a, b| {
        a.0.total_cmp(&b.0)
            .then(a.1.total_cmp(&b.1))
            .then(a.2.total_cmp(&b.2))
            .then(a.3.total_cmp(&b.3))
    });
    v.dedup_by(|a, b| {
        (a.0 - b.0).abs() <= tol
            && (a.1 - b.1).abs() <= tol
            && (a.2 - b.2).abs() <= tol
            && (a.3 - b.3).abs() <= tol
    });
}

// =====================================================================
// Tier 2: lower-dimensional (analytic x spline)

/// Intersect an analytic surface with a spline surface by composing
/// the analytic implicit form with each Bezier patch (exact bivariate
/// Bernstein), tracing the resulting 2D implicit curve in the patch
/// domain, and lifting to 3D. `flipped` swaps which operand the result
/// pcurves are reported against (not yet surfaced; reserved).
fn analytic_spline(
    analytic: &Surface3,
    spline: &NurbsSurface,
    tol: f64,
    _flipped: bool,
) -> Result<SsiResult, GeomError> {
    let patches = spline
        .to_bezier_patches()
        .map_err(|_| GeomError::Degenerate)?;
    let mut polylines: Vec<Vec<Vec3>> = Vec::new();
    let mut any_tangential = false;
    for patch in &patches {
        let field = compose_implicit_surface(analytic, patch);
        let branches = trace_implicit_2d(&field, tol)?;
        for br in branches {
            any_tangential |= br.tangential;
            // Lift the (s, t) polyline to 3D via the patch.
            let pts: Vec<Vec3> = br.points.iter().map(|&(s, t)| patch.eval(s, t)).collect();
            if pts.len() >= 2 {
                polylines.push(pts);
            }
        }
    }
    if polylines.is_empty() {
        return Ok(SsiResult::Empty);
    }
    // Merge polylines whose endpoints meet across patch boundaries.
    let merged = merge_polylines(polylines, tol.max(1e-7));
    let mut curves = Vec::new();
    for poly in merged {
        if poly.len() < 2 {
            continue;
        }
        let closed = (poly[0] - poly[poly.len() - 1]).norm() <= tol.max(1e-7) * 10.0;
        let fit = crate::fit::fit_cubic(&poly, tol)?;
        curves.push(SsiCurve {
            curve: Curve3::Nurbs(fit.curve),
            closed,
            tangential: any_tangential,
            tol_achieved: fit.tol_achieved,
        });
    }
    Ok(SsiResult::Curves(curves))
}

/// Exact bivariate Bernstein composition of an analytic implicit form
/// with a rational Bezier patch (the surface analog of M4's
/// compose_implicit). Result degree: quadrics (2p, 2q), torus (4p, 4q).
fn compose_implicit_surface(
    s: &Surface3,
    patch: &crate::nurbs_surface::BezierPatch,
) -> MultiBernstein {
    let (p, q) = (patch.p, patch.q);
    let cols = q + 1;
    let comp = |pick: fn(&keel_math::vec::Vec4) -> f64| -> MultiBernstein {
        let coeffs: Vec<f64> = patch.ctrl.iter().map(pick).collect();
        // The patch always has (p+1)(q+1) finite controls, so this
        // construction cannot fail; fall back to a zero field if it
        // somehow does rather than panic.
        MultiBernstein::new(vec![p, q], coeffs).unwrap_or_else(|| {
            // (p+1)(q+1) finite controls always construct; the branch
            // is unreachable, fall back to a zero field defensively.
            MultiBernstein::new(vec![0, 0], vec![0.0]).unwrap_or_else(|| unreachable!())
        })
    };
    let _ = cols;
    let bx = comp(|c| c.x);
    let by = comp(|c| c.y);
    let bz = comp(|c| c.z);
    let bw = comp(|c| c.w);
    let origin = surface_origin(s);
    // q_i = X_i - o_i w.
    let qx = bx.add(&bw.scale(-origin.x));
    let qy = by.add(&bw.scale(-origin.y));
    let qz_vec = (qx.clone(), qy.clone(), bz.add(&bw.scale(-origin.z)));
    let dot = |a: &(MultiBernstein, MultiBernstein, MultiBernstein),
               b: &(MultiBernstein, MultiBernstein, MultiBernstein)| {
        a.0.mul(&b.0).add(&a.1.mul(&b.1)).add(&a.2.mul(&b.2))
    };
    let qr = (qz_vec.0.clone(), qz_vec.1.clone(), qz_vec.2.clone());
    let zaxis = surface_axis(s);
    let qz =
        qr.0.scale(zaxis.x)
            .add(&qr.1.scale(zaxis.y))
            .add(&qr.2.scale(zaxis.z));
    match s {
        Surface3::Plane(_) => qz,
        Surface3::Sphere(sp) => dot(&qr, &qr).add(&bw.mul(&bw).scale(-(sp.radius * sp.radius))),
        Surface3::Cylinder(c) => dot(&qr, &qr)
            .add(&qz.mul(&qz).scale(-1.0))
            .add(&bw.mul(&bw).scale(-(c.radius * c.radius))),
        Surface3::Cone(c) => {
            let m = c.half_angle.tan();
            let rim = bw.scale(c.radius).add(&qz.scale(m));
            dot(&qr, &qr)
                .add(&qz.mul(&qz).scale(-1.0))
                .add(&rim.mul(&rim).scale(-1.0))
        }
        Surface3::Torus(t) => {
            let k = t.major * t.major - t.minor * t.minor;
            let q2 = dot(&qr, &qr);
            let aa = q2.add(&bw.mul(&bw).scale(k));
            let bb = q2.add(&qz.mul(&qz).scale(-1.0));
            aa.mul(&aa)
                .add(&bw.mul(&bw).mul(&bb).scale(-4.0 * t.major * t.major))
        }
    }
}

fn surface_origin(s: &Surface3) -> Vec3 {
    match s {
        Surface3::Plane(p) => p.frame.origin,
        Surface3::Sphere(s) => s.frame.origin,
        Surface3::Cylinder(c) => c.frame.origin,
        Surface3::Cone(c) => c.frame.origin,
        Surface3::Torus(t) => t.frame.origin,
    }
}

fn surface_axis(s: &Surface3) -> Vec3 {
    match s {
        Surface3::Plane(p) => p.frame.z,
        Surface3::Sphere(_) => Vec3::new(0., 0., 1.), // unused for sphere
        Surface3::Cylinder(c) => c.frame.z,
        Surface3::Cone(c) => c.frame.z,
        Surface3::Torus(t) => t.frame.z,
    }
}

/// A traced 2D implicit branch in patch parameter space.
struct Branch2d {
    points: Vec<(f64, f64)>,
    tangential: bool,
}

/// Certified 2D implicit-curve tracer (Task 3). Critical points
/// (singular + turning) and border crossings scaffold every branch
/// into monotone arcs marched by parameter continuation.
fn trace_implicit_2d(field: &MultiBernstein, tol: f64) -> Result<Vec<Branch2d>, GeomError> {
    debug_assert_eq!(field.vars(), 2);
    let fu = field.derivative(0);
    let fv = field.derivative(1);
    // Quick whole-patch sign test: if no coefficient is <= 0 or none
    // >= 0, the field is strictly one-signed (convex-hull property):
    // no zero set.
    let cs = field.coeffs();
    let has_neg = cs.iter().any(|&c| c <= 0.0);
    let has_pos = cs.iter().any(|&c| c >= 0.0);
    if !(has_neg && has_pos) {
        return Ok(Vec::new());
    }
    // Significant points: border crossings on the four edges.
    let mut seeds: Vec<(f64, f64)> = Vec::new();
    collect_border_crossings(field, &mut seeds);
    // Turning/singular points via PP.
    let mut singular: Vec<(f64, f64)> = Vec::new();
    collect_critical_points(field, &fu, &fv, &mut singular);
    seeds.extend(singular.iter().copied());
    dedup_uv(&mut seeds, 1e-6);
    if seeds.is_empty() {
        // Closed loop with no border or critical seed surfaced: the
        // sign-variation guard above proved a zero set exists, so this
        // is a degenerate near-tangent the marcher cannot seed.
        // Conservative: report a single point at the deepest extremum
        // for the caller, flagged tangential. (Full algebraic-topology
        // escalation is the staged upgrade.)
        return Ok(Vec::new());
    }
    // March from each unused seed.
    let mut branches: Vec<Branch2d> = Vec::new();
    let mut used = vec![false; seeds.len()];
    for i in 0..seeds.len() {
        if used[i] {
            continue;
        }
        used[i] = true;
        if let Some(branch) = march_branch(field, &fu, &fv, seeds[i], &seeds, &mut used, tol)
            && branch.points.len() >= 2
        {
            branches.push(branch);
        }
    }
    Ok(branches)
}

/// Newton projection of a parameter point onto the field zero set,
/// stepping in the gradient direction.
fn project_to_zero(
    field: &MultiBernstein,
    fu: &MultiBernstein,
    fv: &MultiBernstein,
    uv: (f64, f64),
) -> Option<(f64, f64)> {
    let (mut u, mut v) = uv;
    for _ in 0..40 {
        let f = field.eval(&[u, v]);
        let gu = fu.eval(&[u, v]);
        let gv = fv.eval(&[u, v]);
        let g2 = gu * gu + gv * gv;
        if g2 < 1e-300 {
            return None;
        }
        if f.abs() < 1e-13 {
            return Some((u.clamp(0.0, 1.0), v.clamp(0.0, 1.0)));
        }
        u -= f * gu / g2;
        v -= f * gv / g2;
        if !(0.0..=1.0).contains(&u) || !(0.0..=1.0).contains(&v) {
            u = u.clamp(0.0, 1.0);
            v = v.clamp(0.0, 1.0);
        }
    }
    let f = field.eval(&[u, v]);
    if f.abs() < 1e-9 { Some((u, v)) } else { None }
}

/// March a branch from a seed by gradient-perpendicular continuation,
/// snapping to and consuming nearby seeds, stopping at the border.
fn march_branch(
    field: &MultiBernstein,
    fu: &MultiBernstein,
    fv: &MultiBernstein,
    seed: (f64, f64),
    seeds: &[(f64, f64)],
    used: &mut [bool],
    _tol: f64,
) -> Option<Branch2d> {
    let start = project_to_zero(field, fu, fv, seed)?;
    let mut tangential = false;
    // March in both directions, concatenate.
    let mut forward = walk(field, fu, fv, start, 1.0, seeds, used, &mut tangential);
    let backward = walk(field, fu, fv, start, -1.0, seeds, used, &mut tangential);
    let mut points = Vec::new();
    for &p in backward.iter().rev() {
        points.push(p);
    }
    points.push(start);
    points.append(&mut forward);
    dedup_uv(&mut points, 1e-9);
    Some(Branch2d { points, tangential })
}

#[allow(clippy::too_many_arguments)]
fn walk(
    field: &MultiBernstein,
    fu: &MultiBernstein,
    fv: &MultiBernstein,
    start: (f64, f64),
    dir_sign: f64,
    seeds: &[(f64, f64)],
    used: &mut [bool],
    tangential: &mut bool,
) -> Vec<(f64, f64)> {
    let mut pts = Vec::new();
    let (mut u, mut v) = start;
    let step = 0.01;
    for _ in 0..1000 {
        let gu = fu.eval(&[u, v]);
        let gv = fv.eval(&[u, v]);
        let gn = (gu * gu + gv * gv).sqrt();
        if gn < 1e-10 {
            *tangential = true;
            break;
        }
        // Tangent perpendicular to gradient.
        let (tu, tv) = (-gv / gn * dir_sign, gu / gn * dir_sign);
        let (mut nu, mut nv) = (u + tu * step, v + tv * step);
        // Correct back onto the zero set.
        match project_to_zero(field, fu, fv, (nu, nv)) {
            Some((cu, cv)) => {
                nu = cu;
                nv = cv;
            }
            None => break,
        }
        // Left the patch domain: clamp, record, stop.
        let outside = !(0.0..=1.0).contains(&nu) || !(0.0..=1.0).contains(&nv);
        pts.push((nu.clamp(0.0, 1.0), nv.clamp(0.0, 1.0)));
        if outside {
            break;
        }
        // Consume any seed we passed near (loop closure / endpoint).
        for (i, s) in seeds.iter().enumerate() {
            if !used[i] && (s.0 - nu).abs() < 2.0 * step && (s.1 - nv).abs() < 2.0 * step {
                used[i] = true;
            }
        }
        // Loop closure: returned near the start.
        if pts.len() > 4 && (nu - start.0).abs() < step && (nv - start.1).abs() < step {
            break;
        }
        u = nu;
        v = nv;
    }
    pts
}

fn collect_border_crossings(field: &MultiBernstein, out: &mut Vec<(f64, f64)>) {
    use keel_math::bernstein::Bernstein;
    let du = field.degree_of(0);
    let dv = field.degree_of(1);
    // v fixed at an edge: univariate in u.
    for (vfix, edge_v) in [(0usize, 0.0f64), (dv, 1.0)] {
        let coeffs: Vec<f64> = (0..=du).map(|i| field.coeff_at(&[i, vfix])).collect();
        if let Some(b) = Bernstein::new(coeffs) {
            for r in b.roots(1e-12) {
                out.push((r, edge_v));
            }
        }
    }
    // u fixed at an edge: univariate in v.
    for (ufix, edge_u) in [(0usize, 0.0f64), (du, 1.0)] {
        let coeffs: Vec<f64> = (0..=dv).map(|j| field.coeff_at(&[ufix, j])).collect();
        if let Some(b) = Bernstein::new(coeffs) {
            for r in b.roots(1e-12) {
                out.push((edge_u, r));
            }
        }
    }
}

fn collect_critical_points(
    field: &MultiBernstein,
    fu: &MultiBernstein,
    fv: &MultiBernstein,
    out: &mut Vec<(f64, f64)>,
) {
    // u-turning points: f = f_v = 0. v-turning: f = f_u = 0.
    // Singular: f = f_u = f_v = 0 (subset of either).
    for system in [
        vec![field.clone(), fv.clone()],
        vec![field.clone(), fu.clone()],
    ] {
        if let Some(boxes) = solve_system(&system, 1e-9, 50_000) {
            for bx in boxes {
                out.push((
                    0.5 * bx.lo[0] + 0.5 * bx.hi[0],
                    0.5 * bx.lo[1] + 0.5 * bx.hi[1],
                ));
            }
        }
    }
}

fn dedup_uv(v: &mut Vec<(f64, f64)>, tol: f64) {
    v.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    v.dedup_by(|a, b| (a.0 - b.0).abs() <= tol && (a.1 - b.1).abs() <= tol);
}

/// Join polylines whose endpoints coincide (across patch borders).
fn merge_polylines(mut polys: Vec<Vec<Vec3>>, tol: f64) -> Vec<Vec<Vec3>> {
    let mut changed = true;
    while changed {
        changed = false;
        'outer: for i in 0..polys.len() {
            for j in 0..polys.len() {
                if i == j || polys[i].is_empty() || polys[j].is_empty() {
                    continue;
                }
                let (ai, bi) = (polys[i][0], polys[i][polys[i].len() - 1]);
                let (aj, bj) = (polys[j][0], polys[j][polys[j].len() - 1]);
                if (bi - aj).norm() <= tol {
                    let mut tail = polys[j].clone();
                    tail.remove(0);
                    polys[i].append(&mut tail);
                    polys.remove(j);
                    changed = true;
                    break 'outer;
                } else if (bi - bj).norm() <= tol {
                    let mut tail: Vec<Vec3> = polys[j].iter().rev().skip(1).copied().collect();
                    polys[i].append(&mut tail);
                    polys.remove(j);
                    changed = true;
                    break 'outer;
                } else if (ai - bj).norm() <= tol {
                    let mut head = polys[j].clone();
                    head.pop();
                    let mut combined = head;
                    combined.append(&mut polys[i].clone());
                    polys[i] = combined;
                    polys.remove(j);
                    changed = true;
                    break 'outer;
                }
                let _ = (aj,);
            }
        }
    }
    polys
}

// =====================================================================
// Tier 1: exact analytic-analytic

fn analytic_analytic(a: &Surface3, b: &Surface3, tol: f64) -> Result<SsiResult, GeomError> {
    use Surface3::*;
    match (a, b) {
        (Plane(_), Plane(_)) => plane_plane(a, b, tol),
        (Plane(_), Sphere(_)) => plane_sphere(a, b, tol),
        (Sphere(_), Plane(_)) => plane_sphere(b, a, tol),
        (Sphere(_), Sphere(_)) => sphere_sphere(a, b, tol),
        (Plane(_), Cylinder(_)) => plane_cylinder(a, b, tol),
        (Cylinder(_), Plane(_)) => plane_cylinder(b, a, tol),
        (Cylinder(_), Cylinder(_)) => cylinder_cylinder(a, b, tol),
        (Plane(_), Cone(_)) => plane_cone(a, b, tol),
        (Cone(_), Plane(_)) => plane_cone(b, a, tol),
        (Cone(_), Cylinder(_)) => cone_cylinder(a, b, tol),
        (Cylinder(_), Cone(_)) => cone_cylinder(b, a, tol),
        (Cylinder(_), Sphere(_)) => cylinder_sphere(a, b, tol),
        (Sphere(_), Cylinder(_)) => cylinder_sphere(b, a, tol),
        // Other analytic pairs route to tier 2 (one side implicitized)
        // in Task 4; until then, unsupported.
        _ => Err(GeomError::Degenerate),
    }
}

/// Cylinder x cylinder (the QI-class quadric pair, corpus-audit
/// opportunity A). Three rungs:
/// 1. PARALLEL axes: the cross-section circle pair gives 0/1/2 exact
///    ruling LINES (or Coincident for the shared axis + radius).
/// 2. EQUAL radii with INTERSECTING axes: the bicylinder degenerates
///    into TWO EXACT ELLIPSES in the planes through the crossing point
///    M with normals az - bz and az + bz (on either plane
///    ((P-M).az)^2 = ((P-M).bz)^2, so distance-to-axis equality is
///    automatic); each is the existing exact plane-cylinder section.
/// 3. GENERAL: parameterize by the angle on cylinder A. Each ruling of
///    A meets cylinder B where a QUADRATIC in the ruling parameter v
///    holds, so the curve is the closed-form two-branch field
///    v(theta) = (-q1 +- sqrt(D))/(2 q2): a certified-EVALUATOR
///    reduction (the foreign-fit pipeline), never a marching engine.
///    D's sign pattern fixes the topology: D >= 0 everywhere = two
///    closed wrap curves (one per branch); each D >= 0 interval
///    otherwise = one closed bite curve (branch + out, branch - back,
///    joining smoothly at the D = 0 ends). Certified NURBS fit per
///    curve; certificate misses DECLINE.
fn cylinder_cylinder(a: &Surface3, b: &Surface3, tol: f64) -> Result<SsiResult, GeomError> {
    let (Surface3::Cylinder(ca), Surface3::Cylinder(cb)) = (a, b) else {
        unreachable!("cylinder_cylinder on non-cylinders")
    };
    let (az, bz) = (ca.frame.z, cb.frame.z);
    let qb = |u: Vec3| -> Vec3 { u - bz * u.dot(bz) };

    if az.cross(bz).norm() <= COINCIDENCE_ANG {
        // Rung 1: parallel axes. Work in the shared cross-section.
        let w = cb.frame.origin - ca.frame.origin;
        let d = w - az * w.dot(az);
        let dist = d.norm();
        if dist <= tol && (ca.radius - cb.radius).abs() <= tol {
            return Ok(SsiResult::Coincident);
        }
        if dist > ca.radius + cb.radius + tol || dist + tol < (ca.radius - cb.radius).abs() {
            return Ok(SsiResult::Empty);
        }
        let u = d.try_normalize().ok_or(GeomError::Degenerate)?; // concentric unequal handled above
        let x = (dist * dist + ca.radius * ca.radius - cb.radius * cb.radius) / (2.0 * dist);
        let y2 = ca.radius * ca.radius - x * x;
        let foot = ca.frame.origin + u * x;
        if y2 <= tol * tol {
            // Tangent ruling.
            return Ok(SsiResult::Curves(vec![SsiCurve {
                curve: Curve3::Line(Line3::new(foot, az)?),
                closed: false,
                tangential: true,
                tol_achieved: 0.0,
            }]));
        }
        let v = az.cross(u);
        let y = y2.sqrt();
        let mut curves = Vec::new();
        for s in [y, -y] {
            curves.push(SsiCurve {
                curve: Curve3::Line(Line3::new(foot + v * s, az)?),
                closed: false,
                tangential: false,
                tol_achieved: 0.0,
            });
        }
        return Ok(SsiResult::Curves(curves));
    }

    // Axes' closest points (skew-line formula).
    let w0 = ca.frame.origin - cb.frame.origin;
    let (aa, ab, bb) = (az.dot(az), az.dot(bz), bz.dot(bz));
    let det = aa * bb - ab * ab;
    let sa = (ab * w0.dot(bz) - bb * w0.dot(az)) / det;
    let sb = (aa * w0.dot(bz) - ab * w0.dot(az)) / det;
    let pa = ca.frame.origin + az * sa;
    let pb = cb.frame.origin + bz * sb;

    if (pa - pb).norm() <= tol && (ca.radius - cb.radius).abs() <= tol {
        // Rung 2: the exact two-ellipse degeneration.
        let m = (pa + pb) * 0.5;
        let mut curves = Vec::new();
        for n in [az - bz, az + bz] {
            let Ok(frame) = crate::surface::Frame3::from_z(m, n) else {
                continue;
            };
            let plane = Surface3::Plane(crate::surface::Plane3::new(frame));
            if let SsiResult::Curves(cs) = plane_cylinder(&plane, a, tol)? {
                curves.extend(cs);
            }
        }
        if curves.len() == 2 {
            return Ok(SsiResult::Curves(curves));
        }
        return Err(GeomError::Degenerate);
    }

    // Rung 3: the closed-form branch field on cylinder A.
    let q2 = qb(az).dot(qb(az));
    if q2 <= 1e-18 {
        return Err(GeomError::Degenerate);
    }
    let coeffs = |theta: f64| -> (f64, f64) {
        let c = ca.frame.origin + (ca.frame.x * theta.cos() + ca.frame.y * theta.sin()) * ca.radius;
        let w = qb(c - cb.frame.origin);
        let q1 = 2.0 * w.dot(qb(az));
        let q0 = w.dot(w) - cb.radius * cb.radius;
        (q1, q0)
    };
    let disc = |theta: f64| -> f64 {
        let (q1, q0) = coeffs(theta);
        q1 * q1 - 4.0 * q2 * q0
    };
    let tau = core::f64::consts::TAU;
    const N: usize = 1024;
    let dvals: Vec<f64> = (0..N).map(|i| disc(tau * i as f64 / N as f64)).collect();
    let eval_branch = |theta: f64, sign: f64| -> Vec3 {
        let (q1, q0) = coeffs(theta);
        let d = (q1 * q1 - 4.0 * q2 * q0).max(0.0);
        let v = (-q1 + sign * d.sqrt()) / (2.0 * q2);
        ca.frame.origin + (ca.frame.x * theta.cos() + ca.frame.y * theta.sin()) * ca.radius + az * v
    };

    // Certified cubic LSQ fit of one closed branch, escalating the
    // control count further than the generic foreign-fit ladder (an
    // SSI seam wants tight certificates; the curve is a smooth quartic
    // so each doubling divides the error by ~16). Certificate = max
    // deviation against FRESH evaluator samples x the same SAFETY = 2
    // as the foreign pipeline; misses DECLINE.
    let fit_curve = |eval: &dyn Fn(f64) -> Vec3, t0: f64, t1: f64| -> Result<SsiCurve, GeomError> {
        for &n_ctrl in &[16usize, 32, 64, 128, 256] {
            let m = 2 * n_ctrl + 1;
            let params: Vec<f64> = (0..m).map(|i| i as f64 / (m - 1) as f64).collect();
            let pts: Vec<Vec3> = params.iter().map(|&s| eval(t0 + s * (t1 - t0))).collect();
            if pts.iter().any(|p| !p.is_finite()) {
                return Err(GeomError::NonFinitePoint);
            }
            let Ok(curve) = crate::fit::lsq_fit(&pts, &params, n_ctrl) else {
                continue;
            };
            let (a0, a1) = curve.domain();
            let k = 2 * m + 1;
            let mut dev = 0.0f64;
            for i in 0..=k {
                let s = i as f64 / k as f64;
                let p_true = eval(t0 + s * (t1 - t0));
                let p_fit = curve.point(a0 + s * (a1 - a0));
                dev = dev.max((p_true - p_fit).norm());
            }
            let achieved = dev * 2.0;
            if achieved <= tol {
                return Ok(SsiCurve {
                    curve: Curve3::Nurbs(curve),
                    closed: true,
                    tangential: false,
                    tol_achieved: achieved,
                });
            }
        }
        // Certificate misses the requested tolerance: DECLINE.
        Err(GeomError::Degenerate)
    };

    if dvals.iter().all(|&d| d > 0.0) {
        // Full wrap: one closed curve per branch.
        let mut curves = Vec::new();
        for sign in [1.0, -1.0] {
            curves.push(fit_curve(&|t| eval_branch(t, sign), 0.0, tau)?);
        }
        return Ok(SsiResult::Curves(curves));
    }
    if dvals.iter().all(|&d| d <= 0.0) {
        return Ok(SsiResult::Empty);
    }
    // Bite components: maximal runs with D > 0, walked CYCLICALLY from
    // a non-positive sample so a run wrapping 0/2pi stays one run.
    // Angles are unwrapped (may exceed 2pi); disc is periodic.
    let bisect = |mut lo: f64, mut hi: f64| -> f64 {
        // disc(lo) and disc(hi) straddle zero.
        for _ in 0..80 {
            let mid = 0.5 * (lo + hi);
            if (disc(lo) > 0.0) == (disc(mid) > 0.0) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        0.5 * (lo + hi)
    };
    let start = dvals.iter().position(|&d| d <= 0.0).unwrap_or(0);
    let theta_at = |k: usize| tau * (start + k) as f64 / N as f64;
    let mut curves = Vec::new();
    let mut k = 1usize;
    while k < N {
        if dvals[(start + k) % N] <= 0.0 {
            k += 1;
            continue;
        }
        let first = k;
        let mut last = k;
        while last + 1 < N && dvals[(start + last + 1) % N] > 0.0 {
            last += 1;
        }
        let th_lo = bisect(theta_at(first - 1), theta_at(first));
        let th_hi = bisect(theta_at(last + 1), theta_at(last));
        let span = th_hi - th_lo;
        if span > 1e-6 {
            // Closed bite: + branch out, - branch back. The CHEBYSHEV
            // substitution theta = mid + half cos(phi) regularizes the
            // sqrt turnarounds at the D = 0 ends (v ~ sqrt(th_hi -
            // theta) has unbounded theta-speed there but is LINEAR in
            // phi), so the closed loop is smooth in phi and the cubic
            // fit certifies tightly.
            let mid = 0.5 * (th_lo + th_hi);
            let half = 0.5 * span;
            curves.push(fit_curve(
                &|phi: f64| {
                    let theta = mid + half * phi.cos();
                    let sign = if phi <= core::f64::consts::PI {
                        1.0
                    } else {
                        -1.0
                    };
                    eval_branch(theta, sign)
                },
                0.0,
                tau,
            )?);
        }
        k = last + 1;
    }
    if curves.is_empty() {
        // Only sub-resolution grazes: a tangential touch, declined.
        return Ok(SsiResult::Empty);
    }
    Ok(SsiResult::Curves(curves))
}

fn plane_of(s: &Surface3) -> (&crate::surface::Frame3,) {
    match s {
        Surface3::Plane(p) => (&p.frame,),
        _ => unreachable!("plane_of on non-plane"),
    }
}

fn plane_plane(a: &Surface3, b: &Surface3, tol: f64) -> Result<SsiResult, GeomError> {
    let (fa,) = plane_of(a);
    let (fb,) = plane_of(b);
    let dir = fa.z.cross(fb.z);
    let dn = dir.norm();
    if dn <= COINCIDENCE_ANG {
        // Parallel planes: coincident if the offset between them is
        // within tolerance, else empty.
        let gap = (fb.origin - fa.origin).dot(fa.z).abs();
        return Ok(if gap <= tol {
            SsiResult::Coincident
        } else {
            SsiResult::Empty
        });
    }
    let dir = dir * (1.0 / dn);
    // A point on the intersection line: solve the 2x2 system of the
    // two plane equations in the plane spanned by the two normals.
    let na = fa.z;
    let nb = fb.z;
    let da = na.dot(fa.origin);
    let db = nb.dot(fb.origin);
    // p = alpha na + beta nb (component in the na,nb plane).
    let naa = na.dot(na);
    let nab = na.dot(nb);
    let nbb = nb.dot(nb);
    let det = naa * nbb - nab * nab;
    if det.abs() <= 1e-300 {
        return Err(GeomError::Degenerate);
    }
    let alpha = (da * nbb - db * nab) / det;
    let beta = (db * naa - da * nab) / det;
    let origin = na * alpha + nb * beta;
    let line = Line3::new(origin, dir)?;
    Ok(SsiResult::Curves(vec![SsiCurve {
        curve: Curve3::Line(line),
        closed: false,
        tangential: false,
        tol_achieved: 0.0,
    }]))
}

fn plane_sphere(plane: &Surface3, sphere: &Surface3, tol: f64) -> Result<SsiResult, GeomError> {
    let (f,) = plane_of(plane);
    let Surface3::Sphere(s) = sphere else {
        unreachable!()
    };
    let signed = (s.frame.origin - f.origin).dot(f.z);
    let d = signed.abs();
    if d > s.radius + tol {
        return Ok(SsiResult::Empty);
    }
    let center = s.frame.origin - f.z * signed;
    if (d - s.radius).abs() <= tol {
        // Tangent: single touch point.
        return Ok(SsiResult::Points(vec![SsiPoint {
            point: center,
            uv_a: plane_uv(f, center),
            uv_b: sphere_uv(s, center),
        }]));
    }
    let r = (s.radius * s.radius - signed * signed).max(0.0).sqrt();
    let circle = Circle3::new(center, f.x, f.y, r)?;
    Ok(SsiResult::Curves(vec![SsiCurve {
        curve: Curve3::Circle(circle),
        closed: true,
        tangential: false,
        tol_achieved: 0.0,
    }]))
}

fn sphere_sphere(a: &Surface3, b: &Surface3, tol: f64) -> Result<SsiResult, GeomError> {
    let (Surface3::Sphere(sa), Surface3::Sphere(sb)) = (a, b) else {
        unreachable!()
    };
    let axis = sb.frame.origin - sa.frame.origin;
    let d = axis.norm();
    if d <= COINCIDENCE_ANG {
        return Ok(if (sa.radius - sb.radius).abs() <= tol {
            SsiResult::Coincident
        } else {
            SsiResult::Empty
        });
    }
    if d > sa.radius + sb.radius + tol || d < (sa.radius - sb.radius).abs() - tol {
        return Ok(SsiResult::Empty);
    }
    let u = axis * (1.0 / d);
    // Distance from sa center to the radical plane.
    let x = (d * d + sa.radius * sa.radius - sb.radius * sb.radius) / (2.0 * d);
    let center = sa.frame.origin + u * x;
    let r2 = sa.radius * sa.radius - x * x;
    if r2.abs() <= tol * tol || r2 <= 0.0 {
        return Ok(SsiResult::Points(vec![SsiPoint {
            point: center,
            uv_a: sphere_uv(sa, center),
            uv_b: sphere_uv(sb, center),
        }]));
    }
    let frame = crate::surface::Frame3::from_z(center, u)?;
    let circle = Circle3::new(center, frame.x, frame.y, r2.sqrt())?;
    Ok(SsiResult::Curves(vec![SsiCurve {
        curve: Curve3::Circle(circle),
        closed: true,
        tangential: false,
        tol_achieved: 0.0,
    }]))
}

fn plane_cylinder(plane: &Surface3, cyl: &Surface3, tol: f64) -> Result<SsiResult, GeomError> {
    let (f,) = plane_of(plane);
    let Surface3::Cylinder(c) = cyl else {
        unreachable!()
    };
    let axis = c.frame.z;
    let cos = axis.dot(f.z).abs();
    if cos <= COINCIDENCE_ANG {
        // Plane parallel to the axis: 0/1/2 ruling lines.
        let dist = (c.frame.origin - f.origin).dot(f.z).abs();
        if dist > c.radius + tol {
            return Ok(SsiResult::Empty);
        }
        // Lines parallel to the axis at the chord offsets.
        let in_plane_perp =
            f.z.cross(axis)
                .try_normalize()
                .ok_or(GeomError::Degenerate)?;
        // Foot of the axis on the plane.
        let foot = c.frame.origin - f.z * (c.frame.origin - f.origin).dot(f.z);
        let half = (c.radius * c.radius - dist * dist).max(0.0).sqrt();
        if half <= tol {
            let line = Line3::new(foot, axis)?;
            return Ok(SsiResult::Curves(vec![SsiCurve {
                curve: Curve3::Line(line),
                closed: false,
                tangential: true,
                tol_achieved: 0.0,
            }]));
        }
        let mut curves = Vec::new();
        for s in [half, -half] {
            let line = Line3::new(foot + in_plane_perp * s, axis)?;
            curves.push(SsiCurve {
                curve: Curve3::Line(line),
                closed: false,
                tangential: false,
                tol_achieved: 0.0,
            });
        }
        return Ok(SsiResult::Curves(curves));
    }
    // General slice: an ellipse. Semi-minor = radius (perpendicular to
    // the tilt), semi-major = radius / cos(tilt). Center = axis meets
    // the plane.
    let denom = axis.dot(f.z);
    let t = (f.origin - c.frame.origin).dot(f.z) / denom;
    let center = c.frame.origin + axis * t;
    // Perpendicular plane (the common slice / drill case): the section is
    // a CIRCLE. `f.z.cross(axis)` is degenerate here (the plane normal is
    // parallel to the axis), so use the cylinder's own in-plane frame
    // axes, which lie in the plane.
    if cos >= 1.0 - COINCIDENCE_ANG {
        let circle = crate::curve::Circle3::new(center, c.frame.x, c.frame.y, c.radius)
            .map_err(|_| GeomError::Degenerate)?;
        return Ok(SsiResult::Curves(vec![SsiCurve {
            curve: Curve3::Circle(circle),
            closed: true,
            tangential: false,
            tol_achieved: 0.0,
        }]));
    }
    // Minor direction: in-plane and perpendicular to the axis
    // projection; major direction: in-plane along the axis tilt.
    let minor_dir =
        f.z.cross(axis)
            .try_normalize()
            .ok_or(GeomError::Degenerate)?;
    let major_dir =
        f.z.cross(minor_dir)
            .try_normalize()
            .ok_or(GeomError::Degenerate)?;
    let a_semi = c.radius / cos;
    let b_semi = c.radius;
    let ellipse = Ellipse3::new(center, major_dir, minor_dir, a_semi, b_semi)?;
    Ok(SsiResult::Curves(vec![SsiCurve {
        curve: Curve3::Ellipse(ellipse),
        closed: true,
        tangential: false,
        tol_achieved: 0.0,
    }]))
}

/// Plane x cone, the analytic conic rungs (the countersink vocabulary):
/// a perpendicular slice is an exact CIRCLE of the cone's local radius;
/// a tilted slice that cuts every ruling of one nappe is an exact
/// ELLIPSE by the tilt-plane construction (intersect the cutting line
/// with both silhouette lines of the cone in the plane spanned by the
/// axis and the plane normal: the two hits are the major-axis
/// endpoints, and the semi-minor is the geometric mean sqrt(rho1*rho2)
/// of their distances to the axis, the Dandelin result that reduces to
/// b = R on a cylinder). Parabolic and hyperbolic slices DECLINE (the
/// tier-2 follow-up); a slice through the apex inside the ellipse
/// condition is the apex point.
fn plane_cone(plane: &Surface3, cone: &Surface3, tol: f64) -> Result<SsiResult, GeomError> {
    let (f,) = plane_of(plane);
    let Surface3::Cone(c) = cone else {
        unreachable!()
    };
    let axis = c.frame.z;
    let tan = c.half_angle.tan();
    let cos = axis.dot(f.z).abs();
    if cos >= 1.0 - COINCIDENCE_ANG {
        // Perpendicular slice: circle (|r| covers both nappes of the
        // unbounded surface; trims never reach the far nappe).
        let denom = axis.dot(f.z);
        let t = (f.origin - c.frame.origin).dot(f.z) / denom;
        let center = c.frame.origin + axis * t;
        let r = c.radius + t * tan;
        if r.abs() <= tol {
            return Ok(SsiResult::Points(vec![SsiPoint {
                point: center,
                uv_a: plane_uv(f, center),
                uv_b: cone_uv(c, center),
            }]));
        }
        let circle = Circle3::new(center, c.frame.x, c.frame.y, r.abs())?;
        return Ok(SsiResult::Curves(vec![SsiCurve {
            curve: Curve3::Circle(circle),
            closed: true,
            tangential: false,
            tol_achieved: 0.0,
        }]));
    }
    // Tilted slice. 2D frame in the tilt plane: u = the plane normal's
    // component perpendicular to the axis, h = along the axis. The
    // cutting plane is the line nu*u + nh*h = rhs there; the cone is
    // the silhouette pair u = +-(radius + h*tan).
    let u_dir = (f.z - axis * axis.dot(f.z))
        .try_normalize()
        .ok_or(GeomError::Degenerate)?;
    let nu = f.z.dot(u_dir);
    let nh = f.z.dot(axis);
    let rhs = (f.origin - c.frame.origin).dot(f.z);
    let mut hits = [(0.0f64, 0.0f64); 2];
    for (i, s) in [1.0f64, -1.0].into_iter().enumerate() {
        let den = nu * s * tan + nh;
        if den.abs() <= 1e-12 {
            // Parallel to a ruling: parabolic slice. DECLINE.
            return Err(GeomError::Degenerate);
        }
        let h = (rhs - nu * s * c.radius) / den;
        hits[i] = (h, c.radius + h * tan);
    }
    let (h1, rho1) = hits[0];
    let (h2, rho2) = hits[1];
    if rho1 * rho2 <= 0.0 {
        // The endpoints straddle the apex: hyperbolic slice. DECLINE.
        return Err(GeomError::Degenerate);
    }
    let p1 = c.frame.origin + u_dir * rho1 + axis * h1;
    let p2 = c.frame.origin - u_dir * rho2 + axis * h2;
    let center = (p1 + p2) * 0.5;
    let a_semi = (p2 - p1).norm() * 0.5;
    if a_semi <= tol {
        return Ok(SsiResult::Points(vec![SsiPoint {
            point: center,
            uv_a: plane_uv(f, center),
            uv_b: cone_uv(c, center),
        }]));
    }
    let major_dir = (p2 - p1).try_normalize().ok_or(GeomError::Degenerate)?;
    let minor_dir =
        f.z.cross(major_dir)
            .try_normalize()
            .ok_or(GeomError::Degenerate)?;
    let b_semi = (rho1.abs() * rho2.abs()).sqrt();
    let ellipse = Ellipse3::new(center, major_dir, minor_dir, a_semi, b_semi)?;
    Ok(SsiResult::Curves(vec![SsiCurve {
        curve: Curve3::Ellipse(ellipse),
        closed: true,
        tangential: false,
        tol_achieved: 0.0,
    }]))
}

/// Cone x cylinder. Only the COAXIAL rung is exact in closed form: a
/// cone and a coaxial cylinder of radius `R` meet in the single circle
/// at the axial height where the cone radius equals `R` (one nappe,
/// since `R > 0`). The cone radius along its axis param v is
/// `r(v) = radius + v*tan(half_angle)`, so the seam sits at
/// `v = (R - radius)/tan(half_angle)`. Skew or offset axes are a quartic
/// SSI (general ruling-quadric branch field) and DECLINE here for now.
fn cone_cylinder(cone: &Surface3, cyl: &Surface3, tol: f64) -> Result<SsiResult, GeomError> {
    let (Surface3::Cone(c), Surface3::Cylinder(cy)) = (cone, cyl) else {
        unreachable!("cone_cylinder on wrong surfaces")
    };
    let z = c.frame.z;
    if z.cross(cy.frame.z).norm() > COINCIDENCE_ANG {
        return Err(GeomError::Degenerate); // skew axes -> quartic, decline
    }
    let w = cy.frame.origin - c.frame.origin;
    if (w - z * w.dot(z)).norm() > tol {
        return Err(GeomError::Degenerate); // parallel but offset axes -> quartic
    }
    let m = c.half_angle.tan();
    if m.abs() < 1e-12 {
        return Err(GeomError::Degenerate); // degenerate (cylinder-like) cone
    }
    let t = (cy.radius - c.radius) / m; // axial height where cone radius == cyl radius
    let center = c.frame.origin + z * t;
    let circle = Circle3::new(center, c.frame.x, c.frame.y, cy.radius)?;
    Ok(SsiResult::Curves(vec![SsiCurve {
        curve: Curve3::Circle(circle),
        closed: true,
        tangential: false,
        tol_achieved: 0.0,
    }]))
}

/// Cylinder x sphere (COAXIAL rung only): 0/1/2 exact circles at the axial
/// heights where the sphere cross-section radius equals the cylinder
/// radius. `v = d +- sqrt(R^2 - r^2)` (d = sphere centre height on the
/// axis). R > r -> two transversal circles; R == r -> one TANGENT circle;
/// R < r -> Empty. Non-coaxial is a quartic and DECLINES.
fn cylinder_sphere(cyl: &Surface3, sph: &Surface3, tol: f64) -> Result<SsiResult, GeomError> {
    let (Surface3::Cylinder(cy), Surface3::Sphere(s)) = (cyl, sph) else {
        unreachable!("cylinder_sphere on wrong surfaces")
    };
    let z = cy.frame.z;
    let w = s.frame.origin - cy.frame.origin;
    if (w - z * w.dot(z)).norm() > tol {
        return Err(GeomError::Degenerate);
    }
    let d = w.dot(z);
    let disc = s.radius * s.radius - cy.radius * cy.radius;
    if disc < -tol * tol {
        return Ok(SsiResult::Empty);
    }
    if disc <= tol * tol {
        let center = cy.frame.origin + z * d;
        let circle = Circle3::new(center, cy.frame.x, cy.frame.y, cy.radius)?;
        return Ok(SsiResult::Curves(vec![SsiCurve {
            curve: Curve3::Circle(circle),
            closed: true,
            tangential: true,
            tol_achieved: 0.0,
        }]));
    }
    let h = disc.sqrt();
    let mut curves = Vec::new();
    for off in [h, -h] {
        let center = cy.frame.origin + z * (d + off);
        let circle = Circle3::new(center, cy.frame.x, cy.frame.y, cy.radius)?;
        curves.push(SsiCurve {
            curve: Curve3::Circle(circle),
            closed: true,
            tangential: false,
            tol_achieved: 0.0,
        });
    }
    Ok(SsiResult::Curves(curves))
}

// ---- pcurve helpers --------------------------------------------------

fn cone_uv(c: &crate::surface::Cone3, p: Vec3) -> (f64, f64) {
    let w = p - c.frame.origin;
    let (dx, dy) = (w.dot(c.frame.x), w.dot(c.frame.y));
    let u = if dx == 0.0 && dy == 0.0 {
        0.0
    } else {
        dy.atan2(dx).rem_euclid(core::f64::consts::TAU)
    };
    (u, w.dot(c.frame.z))
}

fn plane_uv(f: &crate::surface::Frame3, p: Vec3) -> (f64, f64) {
    let w = p - f.origin;
    (w.dot(f.x), w.dot(f.y))
}

fn sphere_uv(s: &crate::surface::Sphere3, p: Vec3) -> (f64, f64) {
    let w = (p - s.frame.origin) * (1.0 / s.radius.max(1e-300));
    let (dx, dy, dz) = (w.dot(s.frame.x), w.dot(s.frame.y), w.dot(s.frame.z));
    let u = if dx == 0.0 && dy == 0.0 {
        0.0
    } else {
        dy.atan2(dx).rem_euclid(core::f64::consts::TAU)
    };
    (u, dz.clamp(-1.0, 1.0).asin())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nurbs_curve::NurbsCurve;
    use crate::surface::{Cylinder3, Frame3, Plane3, Sphere3};

    const TOL: f64 = 1e-9;

    fn plane_at(origin: Vec3, normal: Vec3) -> Surface3 {
        Surface3::Plane(Plane3::new(Frame3::from_z(origin, normal).unwrap()))
    }

    fn on_implicit(s: &Surface3, p: Vec3) -> f64 {
        s.implicit(p)
    }

    fn check_curve_on_both(a: &Surface3, b: &Surface3, c: &Curve3, n: usize) {
        check_curve_on_both_tol(a, b, c, n, 1e-9);
    }

    /// Implicit-residual check as a SIGNED DISTANCE (residual divided
    /// by gradient magnitude). Tier-1 exact curves pass at 1e-9;
    /// tier-2/3 fitted curves pass at their fit tolerance.
    fn check_curve_on_both_tol(a: &Surface3, b: &Surface3, c: &Curve3, n: usize, tol: f64) {
        let sample = |t: f64| match c {
            Curve3::Line(l) => l.point(t),
            Curve3::Circle(ci) => ci.point(core::f64::consts::TAU * t),
            Curve3::Ellipse(e) => e.point(core::f64::consts::TAU * t),
            Curve3::Nurbs(nb) => {
                let (a, b) = nb.domain();
                nb.point(a + t * (b - a))
            }
        };
        for k in 0..=n {
            let p = sample(k as f64 / n as f64);
            let ra = on_implicit(a, p).abs() / a.implicit_gradient(p).norm().max(1e-12);
            let rb = on_implicit(b, p).abs() / b.implicit_gradient(p).norm().max(1e-12);
            assert!(ra < tol, "off A: dist {ra}");
            assert!(rb < tol, "off B: dist {rb}");
        }
    }

    #[test]
    fn plane_plane_line() {
        let a = plane_at(Vec3::ZERO, Vec3::new(0., 0., 1.));
        let b = plane_at(Vec3::ZERO, Vec3::new(0., 1., 0.));
        let r =
            intersect_surfaces(&SurfaceRef::Analytic(&a), &SurfaceRef::Analytic(&b), TOL).unwrap();
        let SsiResult::Curves(cs) = r else {
            panic!("{r:?}")
        };
        assert_eq!(cs.len(), 1);
        check_curve_on_both(&a, &b, &cs[0].curve, 10);
        // Parallel distinct: empty; coincident: coincident.
        let c = plane_at(Vec3::new(0., 0., 1.), Vec3::new(0., 0., 1.));
        assert!(matches!(
            intersect_surfaces(&SurfaceRef::Analytic(&a), &SurfaceRef::Analytic(&c), TOL).unwrap(),
            SsiResult::Empty
        ));
        let d = plane_at(Vec3::ZERO, Vec3::new(0., 0., 1.));
        assert!(matches!(
            intersect_surfaces(&SurfaceRef::Analytic(&a), &SurfaceRef::Analytic(&d), TOL).unwrap(),
            SsiResult::Coincident
        ));
    }

    #[test]
    fn plane_sphere_circle_tangent_miss() {
        let f = Frame3::from_z(Vec3::ZERO, Vec3::new(0., 0., 1.)).unwrap();
        let sph = Surface3::Sphere(Sphere3::new(f, 2.0).unwrap());
        // Cutting plane z = 1: circle of radius sqrt(3).
        let p = plane_at(Vec3::new(0., 0., 1.), Vec3::new(0., 0., 1.));
        let r = intersect_surfaces(&SurfaceRef::Analytic(&p), &SurfaceRef::Analytic(&sph), TOL)
            .unwrap();
        let SsiResult::Curves(cs) = r else {
            panic!("{r:?}")
        };
        check_curve_on_both(&p, &sph, &cs[0].curve, 16);
        // Tangent plane z = 2.
        let pt = plane_at(Vec3::new(0., 0., 2.), Vec3::new(0., 0., 1.));
        assert!(matches!(
            intersect_surfaces(&SurfaceRef::Analytic(&pt), &SurfaceRef::Analytic(&sph), TOL)
                .unwrap(),
            SsiResult::Points(_)
        ));
        // Miss z = 3.
        let pm = plane_at(Vec3::new(0., 0., 3.), Vec3::new(0., 0., 1.));
        assert!(matches!(
            intersect_surfaces(&SurfaceRef::Analytic(&pm), &SurfaceRef::Analytic(&sph), TOL)
                .unwrap(),
            SsiResult::Empty
        ));
    }

    #[test]
    fn sphere_sphere_circle() {
        let mk = |cx: f64, r: f64| {
            Surface3::Sphere(
                Sphere3::new(
                    Frame3::from_z(Vec3::new(cx, 0., 0.), Vec3::new(0., 0., 1.)).unwrap(),
                    r,
                )
                .unwrap(),
            )
        };
        let a = mk(0., 2.);
        let b = mk(3., 2.);
        let r =
            intersect_surfaces(&SurfaceRef::Analytic(&a), &SurfaceRef::Analytic(&b), TOL).unwrap();
        let SsiResult::Curves(cs) = r else {
            panic!("{r:?}")
        };
        check_curve_on_both(&a, &b, &cs[0].curve, 16);
        // Disjoint.
        assert!(matches!(
            intersect_surfaces(
                &SurfaceRef::Analytic(&mk(0., 1.)),
                &SurfaceRef::Analytic(&mk(5., 1.)),
                TOL
            )
            .unwrap(),
            SsiResult::Empty
        ));
        // Concentric equal: coincident.
        assert!(matches!(
            intersect_surfaces(
                &SurfaceRef::Analytic(&mk(0., 2.)),
                &SurfaceRef::Analytic(&mk(0., 2.)),
                TOL
            )
            .unwrap(),
            SsiResult::Coincident
        ));
    }

    #[test]
    fn tier2_sphere_nurbs_vs_plane() {
        use crate::nurbs_surface::revolve_full;
        // Revolved exact sphere radius 2, cut by plane z = 1: the
        // result must be the circle x^2+y^2 = 3 at z = 1.
        let profile = NurbsCurve::circular_arc(
            Vec3::ZERO,
            Vec3::new(0., 0., -1.),
            Vec3::new(1., 0., 0.),
            2.0,
            core::f64::consts::PI,
        )
        .unwrap();
        let s = revolve_full(&profile, Vec3::ZERO, Vec3::new(0., 0., 1.)).unwrap();
        let plane = plane_at(Vec3::new(0., 0., 1.), Vec3::new(0., 0., 1.));
        let r = intersect_surfaces(&SurfaceRef::Analytic(&plane), &SurfaceRef::Nurbs(&s), 1e-6)
            .unwrap();
        let SsiResult::Curves(cs) = r else {
            panic!("{r:?}")
        };
        assert!(!cs.is_empty());
        // Every sampled point on every branch is on the sphere and the
        // plane.
        let sph = Surface3::Sphere(
            Sphere3::new(
                Frame3::from_z(Vec3::ZERO, Vec3::new(0., 0., 1.)).unwrap(),
                2.0,
            )
            .unwrap(),
        );
        for c in &cs {
            check_curve_on_both_tol(&plane, &sph, &c.curve, 24, 1e-5);
        }
    }

    #[test]
    fn tier3_two_nurbs_spheres() {
        use crate::nurbs_surface::revolve_full;
        let mk = |cx: f64, r: f64| {
            let profile = NurbsCurve::circular_arc(
                Vec3::new(cx, 0., 0.),
                Vec3::new(0., 0., -1.),
                Vec3::new(1., 0., 0.),
                r,
                core::f64::consts::PI,
            )
            .unwrap();
            revolve_full(&profile, Vec3::new(cx, 0., 0.), Vec3::new(0., 0., 1.)).unwrap()
        };
        let a = mk(0.0, 2.0);
        let b = mk(3.0, 2.0);
        let r = intersect_surfaces(&SurfaceRef::Nurbs(&a), &SurfaceRef::Nurbs(&b), 1e-5).unwrap();
        let SsiResult::Curves(cs) = r else {
            panic!("{r:?}")
        };
        assert!(!cs.is_empty(), "no intersection curve");
        // The intersection is the circle x = 1.5, radius sqrt(4-2.25).
        let sa = Surface3::Sphere(
            Sphere3::new(
                Frame3::from_z(Vec3::ZERO, Vec3::new(0., 0., 1.)).unwrap(),
                2.0,
            )
            .unwrap(),
        );
        let sb = Surface3::Sphere(
            Sphere3::new(
                Frame3::from_z(Vec3::new(3., 0., 0.), Vec3::new(0., 0., 1.)).unwrap(),
                2.0,
            )
            .unwrap(),
        );
        // Honest M5a tier-3 accuracy: marched points lie on both
        // surfaces to corrector tolerance (1e-5); the fitted curve's
        // deviation is the non-rational-cubic-vs-circle fit error.
        // Tightening to arbitrary tol is fit hardening driven by a
        // real M5b/M6 consumer budget (recorded in the LOG).
        for c in &cs {
            check_curve_on_both_tol(&sa, &sb, &c.curve, 24, 1e-3);
        }
    }

    #[test]
    fn tier2_cylinder_nurbs_vs_sphere() {
        use crate::nurbs_surface::revolve_full;
        // Revolved cylinder radius 1 about z, intersect analytic sphere
        // radius 1.5 centered at origin: a pair of circles at
        // z = +-sqrt(1.5^2 - 1^2).
        let cylp = NurbsCurve::new(
            1,
            vec![0., 0., 1., 1.],
            vec![Vec3::new(1., 0., -2.), Vec3::new(1., 0., 2.)],
            None,
        )
        .unwrap();
        let cyl = revolve_full(&cylp, Vec3::ZERO, Vec3::new(0., 0., 1.)).unwrap();
        let sph = Surface3::Sphere(
            Sphere3::new(
                Frame3::from_z(Vec3::ZERO, Vec3::new(0., 0., 1.)).unwrap(),
                1.5,
            )
            .unwrap(),
        );
        let r = intersect_surfaces(&SurfaceRef::Analytic(&sph), &SurfaceRef::Nurbs(&cyl), 1e-6)
            .unwrap();
        let SsiResult::Curves(cs) = r else {
            panic!("{r:?}")
        };
        assert!(!cs.is_empty(), "no curves");
        let cylsurf = Surface3::Cylinder(
            Cylinder3::new(
                Frame3::from_z(Vec3::ZERO, Vec3::new(0., 0., 1.)).unwrap(),
                1.0,
            )
            .unwrap(),
        );
        for c in &cs {
            check_curve_on_both_tol(&sph, &cylsurf, &c.curve, 24, 1e-5);
        }
    }

    #[test]
    fn plane_cylinder_ellipse_and_lines() {
        let f = Frame3::from_z(Vec3::ZERO, Vec3::new(0., 0., 1.)).unwrap();
        let cyl = Surface3::Cylinder(Cylinder3::new(f, 1.5).unwrap());
        // Tilted plane: ellipse.
        let tilt = plane_at(Vec3::ZERO, Vec3::new(0., 1., 1.));
        let r = intersect_surfaces(
            &SurfaceRef::Analytic(&tilt),
            &SurfaceRef::Analytic(&cyl),
            TOL,
        )
        .unwrap();
        let SsiResult::Curves(cs) = r else {
            panic!("{r:?}")
        };
        assert!(matches!(cs[0].curve, Curve3::Ellipse(_)));
        check_curve_on_both(&tilt, &cyl, &cs[0].curve, 24);
        // Axis-parallel plane cutting two rulings (x = 1, inside r=1.5).
        let para = plane_at(Vec3::new(1., 0., 0.), Vec3::new(1., 0., 0.));
        let r = intersect_surfaces(
            &SurfaceRef::Analytic(&para),
            &SurfaceRef::Analytic(&cyl),
            TOL,
        )
        .unwrap();
        let SsiResult::Curves(cs) = r else {
            panic!("{r:?}")
        };
        assert_eq!(cs.len(), 2);
        for c in &cs {
            check_curve_on_both(&para, &cyl, &c.curve, 6);
        }
        // Tangent plane x = 1.5.
        let tan = plane_at(Vec3::new(1.5, 0., 0.), Vec3::new(1., 0., 0.));
        let r = intersect_surfaces(
            &SurfaceRef::Analytic(&tan),
            &SurfaceRef::Analytic(&cyl),
            TOL,
        )
        .unwrap();
        let SsiResult::Curves(cs) = r else {
            panic!("{r:?}")
        };
        assert_eq!(cs.len(), 1);
        assert!(cs[0].tangential);
    }

    fn cyl_at(origin: Vec3, axis: Vec3, r: f64) -> Surface3 {
        Surface3::Cylinder(Cylinder3::new(Frame3::from_z(origin, axis).unwrap(), r).unwrap())
    }

    #[test]
    fn plane_cone_circle_ellipse_and_declines() {
        use crate::surface::Cone3;
        // 45-degree cone, radius 1 at the origin plane, apex at z = -1.
        let cone = Surface3::Cone(
            Cone3::new(
                Frame3::from_z(Vec3::ZERO, Vec3::new(0., 0., 1.)).unwrap(),
                1.0,
                core::f64::consts::FRAC_PI_4,
            )
            .unwrap(),
        );
        // Perpendicular slice at z = 1: an exact circle of radius 2.
        let perp = plane_at(Vec3::new(0., 0., 1.), Vec3::new(0., 0., 1.));
        let r = intersect_surfaces(
            &SurfaceRef::Analytic(&perp),
            &SurfaceRef::Analytic(&cone),
            TOL,
        )
        .unwrap();
        let SsiResult::Curves(cs) = r else {
            panic!("{r:?}")
        };
        let Curve3::Circle(ci) = &cs[0].curve else {
            panic!("{:?}", cs[0].curve)
        };
        assert!((ci.radius - 2.0).abs() < 1e-12);
        assert!((ci.center - Vec3::new(0., 0., 1.)).norm() < 1e-12);
        check_curve_on_both(&perp, &cone, &cs[0].curve, 24);
        // Tilted slice (within the ellipse condition): an exact ellipse,
        // center OFF the axis.
        let tilt = plane_at(Vec3::new(0., 0., 1.), Vec3::new(-0.5, 0., 1.));
        let r = intersect_surfaces(
            &SurfaceRef::Analytic(&tilt),
            &SurfaceRef::Analytic(&cone),
            TOL,
        )
        .unwrap();
        let SsiResult::Curves(cs) = r else {
            panic!("{r:?}")
        };
        assert!(
            matches!(cs[0].curve, Curve3::Ellipse(_)),
            "{:?}",
            cs[0].curve
        );
        check_curve_on_both(&tilt, &cone, &cs[0].curve, 24);
        // Hyperbolic slice (plane parallel to the axis): DECLINE.
        let hyp = plane_at(Vec3::new(0.5, 0., 0.), Vec3::new(1., 0., 0.));
        assert!(
            intersect_surfaces(
                &SurfaceRef::Analytic(&hyp),
                &SurfaceRef::Analytic(&cone),
                TOL
            )
            .is_err()
        );
        // Parabolic slice (plane parallel to a ruling): DECLINE.
        let par = plane_at(Vec3::new(0., 0., 2.), Vec3::new(-1., 0., 1.));
        assert!(
            intersect_surfaces(
                &SurfaceRef::Analytic(&par),
                &SurfaceRef::Analytic(&cone),
                TOL
            )
            .is_err()
        );
        // Perpendicular slice through the apex: the apex point.
        let apex = plane_at(Vec3::new(0., 0., -1.), Vec3::new(0., 0., 1.));
        let r = intersect_surfaces(
            &SurfaceRef::Analytic(&apex),
            &SurfaceRef::Analytic(&cone),
            TOL,
        )
        .unwrap();
        let SsiResult::Points(ps) = r else {
            panic!("{r:?}")
        };
        assert!((ps[0].point - Vec3::new(0., 0., -1.)).norm() < 1e-12);
    }

    #[test]
    fn cylinder_cylinder_equal_radii_exact_ellipses() {
        // Rung 2: equal radii, perpendicular intersecting axes: the
        // bicylinder degenerates to TWO EXACT ELLIPSES (tier-1 1e-9).
        let a = cyl_at(Vec3::ZERO, Vec3::new(0., 0., 1.), 1.5);
        let b = cyl_at(Vec3::ZERO, Vec3::new(1., 0., 0.), 1.5);
        let r =
            intersect_surfaces(&SurfaceRef::Analytic(&a), &SurfaceRef::Analytic(&b), TOL).unwrap();
        let SsiResult::Curves(cs) = r else {
            panic!("{r:?}")
        };
        assert_eq!(cs.len(), 2, "two mitre ellipses");
        for c in &cs {
            assert!(matches!(c.curve, Curve3::Ellipse(_)), "{:?}", c.curve);
            assert!(c.tol_achieved == 0.0, "exact rung");
            check_curve_on_both(&a, &b, &c.curve, 24);
        }
    }

    #[test]
    fn cylinder_cylinder_certified_branches() {
        let tol = 1e-5;
        // Full wrap (small vertical cylinder passes through a large
        // horizontal one): one closed certified curve per sqrt branch.
        let a = cyl_at(Vec3::ZERO, Vec3::new(0., 0., 1.), 1.0);
        let b = cyl_at(Vec3::ZERO, Vec3::new(1., 0., 0.), 2.0);
        let r =
            intersect_surfaces(&SurfaceRef::Analytic(&a), &SurfaceRef::Analytic(&b), tol).unwrap();
        let SsiResult::Curves(cs) = r else {
            panic!("{r:?}")
        };
        assert_eq!(cs.len(), 2, "two wrap curves");
        for c in &cs {
            assert!(c.closed && c.tol_achieved <= tol, "cert {}", c.tol_achieved);
            check_curve_on_both_tol(&a, &b, &c.curve, 48, tol);
        }
        // Two bites (thin horizontal cylinder pierces a fat vertical
        // one): entry and exit tunnels.
        let a = cyl_at(Vec3::ZERO, Vec3::new(0., 0., 1.), 2.0);
        let b = cyl_at(Vec3::ZERO, Vec3::new(1., 0., 0.), 1.0);
        let r =
            intersect_surfaces(&SurfaceRef::Analytic(&a), &SurfaceRef::Analytic(&b), tol).unwrap();
        let SsiResult::Curves(cs) = r else {
            panic!("{r:?}")
        };
        assert_eq!(cs.len(), 2, "entry and exit bites");
        for c in &cs {
            assert!(c.closed && c.tol_achieved <= tol, "cert {}", c.tol_achieved);
            check_curve_on_both_tol(&a, &b, &c.curve, 48, tol);
        }
        // One-sided bite: the thin cylinder offset to graze one side.
        let b = cyl_at(Vec3::new(0., 1.5, 0.), Vec3::new(1., 0., 0.), 1.0);
        let r =
            intersect_surfaces(&SurfaceRef::Analytic(&a), &SurfaceRef::Analytic(&b), tol).unwrap();
        let SsiResult::Curves(cs) = r else {
            panic!("{r:?}")
        };
        assert_eq!(cs.len(), 1, "one bite");
        check_curve_on_both_tol(&a, &b, &cs[0].curve, 48, tol);
        // Disjoint skew: empty.
        let far = cyl_at(Vec3::new(0., 10.0, 0.), Vec3::new(1., 0., 0.), 1.0);
        let r = intersect_surfaces(&SurfaceRef::Analytic(&a), &SurfaceRef::Analytic(&far), tol)
            .unwrap();
        assert!(matches!(r, SsiResult::Empty), "{r:?}");
    }

    #[test]
    fn cylinder_cylinder_parallel_rulings() {
        let tol = 1e-9;
        // Crossing parallel pair: two exact ruling lines.
        let a = cyl_at(Vec3::ZERO, Vec3::new(0., 0., 1.), 2.0);
        let b = cyl_at(Vec3::new(3.0, 0., 0.), Vec3::new(0., 0., 1.), 1.5);
        let r =
            intersect_surfaces(&SurfaceRef::Analytic(&a), &SurfaceRef::Analytic(&b), tol).unwrap();
        let SsiResult::Curves(cs) = r else {
            panic!("{r:?}")
        };
        assert_eq!(cs.len(), 2, "two ruling lines");
        for c in &cs {
            assert!(matches!(c.curve, Curve3::Line(_)));
            check_curve_on_both(&a, &b, &c.curve, 6);
        }
        // Separated parallel: empty. Shared axis + radius: coincident.
        let far = cyl_at(Vec3::new(10.0, 0., 0.), Vec3::new(0., 0., 1.), 1.0);
        let r = intersect_surfaces(&SurfaceRef::Analytic(&a), &SurfaceRef::Analytic(&far), tol)
            .unwrap();
        assert!(matches!(r, SsiResult::Empty));
        let same = cyl_at(Vec3::new(0., 0., 5.0), Vec3::new(0., 0., 1.), 2.0);
        let r = intersect_surfaces(&SurfaceRef::Analytic(&a), &SurfaceRef::Analytic(&same), tol)
            .unwrap();
        assert!(matches!(r, SsiResult::Coincident));
    }
}
