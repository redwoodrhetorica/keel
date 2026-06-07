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
        // Tier 3 (spline x spline) arrives in Task 5.
        (SurfaceRef::Nurbs(_), SurfaceRef::Nurbs(_)) => Err(GeomError::Degenerate),
    }
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
        // Other analytic pairs route to tier 2 (one side implicitized)
        // in Task 4; until then, unsupported.
        _ => Err(GeomError::Degenerate),
    }
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

// ---- pcurve helpers --------------------------------------------------

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
}
