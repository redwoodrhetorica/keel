//! Coincident-face overlap geometry (research file 39 §1): the 2D
//! trim-loop intersection of two coplanar faces, used to imprint the
//! overlap boundary so the on-on classification tables (§2, in
//! boolean.rs) operate on uniformly-classified fragments. Planar/convex
//! this cut; curved carriers and non-convex trims are follow-ups.

use keel_math::vec::Vec3;

/// An orthonormal in-plane basis for a plane with unit normal `n`.
fn plane_basis(n: Vec3) -> (Vec3, Vec3) {
    let seed = if n.x.abs() < 0.9 {
        Vec3::new(1.0, 0.0, 0.0)
    } else {
        Vec3::new(0.0, 1.0, 0.0)
    };
    let u = (seed - n * seed.dot(n)).try_normalize().unwrap_or(seed);
    (u, n.cross(u))
}

/// Clip convex `subject` polygon against convex `clip` polygon
/// (Sutherland-Hodgman) in 2D. Both must be convex; `clip` is taken in
/// counter-clockwise order (we orient it). Returns the intersection
/// polygon (possibly empty).
fn clip_convex(subject: &[[f64; 2]], clip: &[[f64; 2]]) -> Vec<[f64; 2]> {
    // Orient `clip` counter-clockwise.
    let mut clip = clip.to_vec();
    let area2: f64 = (0..clip.len())
        .map(|i| {
            let a = clip[i];
            let b = clip[(i + 1) % clip.len()];
            a[0] * b[1] - b[0] * a[1]
        })
        .sum();
    if area2 < 0.0 {
        clip.reverse();
    }
    let mut output = subject.to_vec();
    let n = clip.len();
    for i in 0..n {
        if output.is_empty() {
            break;
        }
        let a = clip[i];
        let b = clip[(i + 1) % n];
        let edge = [b[0] - a[0], b[1] - a[1]];
        // Inside = left of a->b (CCW clip).
        let inside = |p: [f64; 2]| edge[0] * (p[1] - a[1]) - edge[1] * (p[0] - a[0]) >= -1e-12;
        let isect = |p: [f64; 2], q: [f64; 2]| -> [f64; 2] {
            let d = [q[0] - p[0], q[1] - p[1]];
            let denom = edge[0] * d[1] - edge[1] * d[0];
            if denom.abs() < 1e-300 {
                return q;
            }
            let tt = (edge[0] * (p[1] - a[1]) - edge[1] * (p[0] - a[0])) / -denom;
            [p[0] + d[0] * tt, p[1] + d[1] * tt]
        };
        let input = output.clone();
        output.clear();
        let m = input.len();
        for j in 0..m {
            let cur = input[j];
            let prev = input[(j + m - 1) % m];
            let (ci, pi) = (inside(cur), inside(prev));
            if ci {
                if !pi {
                    output.push(isect(prev, cur));
                }
                output.push(cur);
            } else if pi {
                output.push(isect(prev, cur));
            }
        }
    }
    output
}

/// The boundary segments of the overlap of two coplanar face polygons
/// (`face_a`, `face_b`, both in the common plane of unit normal `n`)
/// that are INTERIOR to `face_a` -- i.e. the cuts that must be imprinted
/// onto face_a to split it into its on-overlap and off-overlap parts.
/// Segments lying on face_a's own boundary are excluded (no cut needed).
/// Returns 3D `(start, end)` pairs.
pub(crate) fn overlap_interior_segments(
    face_a: &[Vec3],
    face_b: &[Vec3],
    n: Vec3,
) -> Vec<(Vec3, Vec3)> {
    if face_a.len() < 3 || face_b.len() < 3 {
        return Vec::new();
    }
    let origin = face_a[0];
    let (u, w) = plane_basis(n);
    let to2 = |p: Vec3| [(p - origin).dot(u), (p - origin).dot(w)];
    let to3 = |q: [f64; 2]| origin + u * q[0] + w * q[1];

    let a2: Vec<[f64; 2]> = face_a.iter().map(|&p| to2(p)).collect();
    let b2: Vec<[f64; 2]> = face_b.iter().map(|&p| to2(p)).collect();
    let overlap = clip_convex(&a2, &b2);
    if overlap.len() < 3 {
        return Vec::new();
    }

    // Is a 2D point on face_a's own boundary?
    let on_boundary = |p: [f64; 2]| -> bool {
        let m = a2.len();
        (0..m)
            .map(|i| poly_edge_dist(p, a2[i], a2[(i + 1) % m]))
            .fold(f64::INFINITY, f64::min)
            < 1e-7
    };

    let mut segs = Vec::new();
    let k = overlap.len();
    for i in 0..k {
        let p = overlap[i];
        let q = overlap[(i + 1) % k];
        let mid = [(p[0] + q[0]) * 0.5, (p[1] + q[1]) * 0.5];
        if !on_boundary(mid) {
            segs.push((to3(p), to3(q)));
        }
    }
    segs
}

/// Do two coplanar face polygons overlap on a positive-area region?
pub(crate) fn coplanar_overlap_exists(face_a: &[Vec3], face_b: &[Vec3], n: Vec3) -> bool {
    if face_a.len() < 3 || face_b.len() < 3 {
        return false;
    }
    let origin = face_a[0];
    let (u, w) = plane_basis(n);
    let to2 = |p: Vec3| [(p - origin).dot(u), (p - origin).dot(w)];
    let a2: Vec<[f64; 2]> = face_a.iter().map(|&p| to2(p)).collect();
    let b2: Vec<[f64; 2]> = face_b.iter().map(|&p| to2(p)).collect();
    clip_convex(&a2, &b2).len() >= 3
}

/// Distance from point `p` to segment `a`-`b` in 2D.
fn poly_edge_dist(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let ab = [b[0] - a[0], b[1] - a[1]];
    let ap = [p[0] - a[0], p[1] - a[1]];
    let len2 = ab[0] * ab[0] + ab[1] * ab[1];
    let t = if len2 < 1e-300 {
        0.0
    } else {
        ((ap[0] * ab[0] + ap[1] * ab[1]) / len2).clamp(0.0, 1.0)
    };
    let c = [a[0] + ab[0] * t, a[1] + ab[1] * t];
    ((p[0] - c[0]).powi(2) + (p[1] - c[1]).powi(2)).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_overlap_interior_cut() {
        // face_a = [0,2]x[0,2] at z=0; face_b = [0,1]x[0,2] (covers the
        // left half). Overlap = [0,1]x[0,2]; the only interior cut is the
        // x=1 edge (the others lie on face_a's boundary).
        let fa = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(2.0, 0.0, 0.0),
            Vec3::new(2.0, 2.0, 0.0),
            Vec3::new(0.0, 2.0, 0.0),
        ];
        let fb = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(1.0, 2.0, 0.0),
            Vec3::new(0.0, 2.0, 0.0),
        ];
        let segs = overlap_interior_segments(&fa, &fb, Vec3::new(0.0, 0.0, 1.0));
        assert_eq!(
            segs.len(),
            1,
            "exactly one interior cut (x=1), got {}",
            segs.len()
        );
        let (s, e) = segs[0];
        // The cut is the x=1 line.
        assert!(
            (s.x - 1.0).abs() < 1e-9 && (e.x - 1.0).abs() < 1e-9,
            "cut not at x=1: {s:?}-{e:?}"
        );
    }

    #[test]
    fn full_overlap_has_no_interior_cut() {
        // Identical faces: overlap = the whole face, no interior cut.
        let f = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(1.0, 1.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        ];
        let segs = overlap_interior_segments(&f, &f, Vec3::new(0.0, 0.0, 1.0));
        assert!(
            segs.is_empty(),
            "identical faces need no cut, got {}",
            segs.len()
        );
    }

    #[test]
    fn disjoint_faces_no_overlap() {
        let fa = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(1.0, 1.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        ];
        let fb = vec![
            Vec3::new(3.0, 0.0, 0.0),
            Vec3::new(4.0, 0.0, 0.0),
            Vec3::new(4.0, 1.0, 0.0),
            Vec3::new(3.0, 1.0, 0.0),
        ];
        let segs = overlap_interior_segments(&fa, &fb, Vec3::new(0.0, 0.0, 1.0));
        assert!(segs.is_empty(), "disjoint faces -> no overlap");
    }
}
