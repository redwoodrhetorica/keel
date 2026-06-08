//! Regularized boolean operations (M6a): union, intersection,
//! difference on solid bodies, for clean transversal configurations.
//!
//! Pipeline (M3 gate section 4): localize -> intersect (SSI, M5a) ->
//! imprint both operands + glue -> classify fragments (PMC, M4) ->
//! select per regularized r-set tables -> stitch + rebuild regions.
//!
//! Scope (M6a, binding): faces meet TRANSVERSALLY. Coplanar/coincident
//! faces, tangencies, and full NURBS-bounded-solid booleans are M6b/M7
//! and surface here as fault tags (partial-success model: a result plus
//! per-pair faults, never all-or-nothing, never a panic).

use crate::body::{Body, TopoError};
use crate::entity::{AnyKey, FaceKey, SurfaceGeom};
use keel_geom::ssi::{SsiResult, SurfaceRef, intersect_surfaces};
use keel_geom::surface::Surface3;

/// The three regularized boolean operations.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BoolOp {
    Union,
    Intersection,
    /// `a` minus `b`.
    Difference,
}

/// A recoverable or fatal fault encountered during a boolean. Faults
/// accumulate; the operation reports partial success rather than
/// aborting the whole result (synthesis A error model).
#[derive(Clone, Debug, PartialEq)]
pub enum BoolFault {
    /// A face pair's surfaces overlap on a region (coplanar/coincident);
    /// deferred to M6b. Carries the two face EntityIds.
    Coincident(u64, u64),
    /// A face pair touches tangentially (SSI returned isolated points);
    /// deferred to M6b.
    Tangent(u64, u64),
    /// SSI failed on a pair (e.g. unsupported surface combination).
    IntersectionFailed(u64, u64),
    /// A topology operation failed during imprint/stitch.
    Topo(TopoError),
    /// The result could not be assembled into a valid body.
    AssemblyFailed(&'static str),
}

/// One intersection curve localized to the face pair that produced it.
#[derive(Clone, Debug)]
pub struct SeamCurve {
    pub face_a: FaceKey,
    pub face_b: FaceKey,
    pub curve: keel_geom::curve::Curve3,
    pub closed: bool,
}

impl Body {
    /// Live face keys in deterministic EntityId order.
    pub(crate) fn face_keys(&self) -> Vec<FaceKey> {
        self.entity_ids()
            .filter_map(|id| match self.lookup(id) {
                Some(AnyKey::Face(k)) => Some(k),
                _ => None,
            })
            .collect()
    }

    /// The analytic surface backing a face, if any (M6a is analytic).
    pub(crate) fn face_surface3(&self, face: FaceKey) -> Option<Surface3> {
        let (sk, _) = self.faces.get(face).and_then(|f| f.surface)?;
        match self.surfaces.get(sk) {
            Some(SurfaceGeom::Analytic(a)) => Some(a.clone()),
            _ => None,
        }
    }
}

/// An imprinted operand: a clone of the input body with the seam
/// curves split into it, plus the seam edges that were created (the
/// boundary along which this operand meets the other).
#[derive(Clone, Debug)]
pub struct ImprintedOperand {
    pub body: Body,
    /// Edges created along the intersection seams (one per seam curve
    /// imprinted on this operand).
    pub seam_edges: Vec<crate::entity::EdgeKey>,
}

/// Imprint the given seam curves onto a clone of `body`, picking each
/// seam's face on this operand via `pick` (face_a for operand A,
/// face_b for operand B). Crossing-free transversal seams only (M6a):
/// closed seams become interior rings, open seams split boundary to
/// boundary. Faults accumulate; a failed imprint leaves that seam out.
fn imprint_operand(
    body: &Body,
    seams: &[SeamCurve],
    pick: impl Fn(&SeamCurve) -> FaceKey,
    tol: f64,
    faults: &mut Vec<BoolFault>,
) -> ImprintedOperand {
    let mut working = body.clone();
    let mut seam_edges = Vec::new();
    for seam in seams {
        let face = pick(seam);
        let res = if seam.closed {
            working.imprint_closed_curve(face, &seam.curve, tol)
        } else {
            working.imprint_open_curve(face, &seam.curve, tol)
        };
        match res {
            Ok(rep) => seam_edges.push(rep.edge),
            Err(e) => faults.push(BoolFault::Topo(e)),
        }
    }
    ImprintedOperand {
        body: working,
        seam_edges,
    }
}

/// Two-body imprint (M3 pipeline steps 1-3, per-operand form): localize
/// + intersect, then imprint the seams onto independent clones of each
/// operand. Each returned body is itself a valid solid (the imprint
/// only splits faces along on-surface curves); they are glued into one
/// result at stitch time, after selection discards the unwanted sides.
pub fn imprint_pair(
    a: &Body,
    b: &Body,
    tol: f64,
) -> (ImprintedOperand, ImprintedOperand, Vec<BoolFault>) {
    let (seams, mut faults) = seam_curves(a, b, tol);
    let ia = imprint_operand(a, &seams, |s| s.face_a, tol, &mut faults);
    let ib = imprint_operand(b, &seams, |s| s.face_b, tol, &mut faults);
    (ia, ib, faults)
}

/// Localize + intersect: every face of `a` against every face of `b`,
/// collecting transversal SSI curves. Coincident/tangent/failed pairs
/// become faults. (All-pairs for M6a's small analytic bodies; AABB/BVH
/// localization is a perf concern deferred until a fuzz/scale need.)
pub fn seam_curves(
    a: &Body,
    b: &Body,
    tol: f64,
) -> (Vec<SeamCurve>, Vec<BoolFault>) {
    let mut seams = Vec::new();
    let mut faults = Vec::new();
    for fa in a.face_keys() {
        let Some(sa) = a.face_surface3(fa) else {
            continue;
        };
        let id_a = a.faces.get(fa).map(|f| f.id.0).unwrap_or(0);
        for fb in b.face_keys() {
            let Some(sb) = b.face_surface3(fb) else {
                continue;
            };
            let id_b = b.faces.get(fb).map(|f| f.id.0).unwrap_or(0);
            match intersect_surfaces(&SurfaceRef::Analytic(&sa), &SurfaceRef::Analytic(&sb), tol) {
                Ok(SsiResult::Curves(cs)) => {
                    for c in cs {
                        if c.tangential {
                            faults.push(BoolFault::Tangent(id_a, id_b));
                            continue;
                        }
                        seams.push(SeamCurve {
                            face_a: fa,
                            face_b: fb,
                            curve: c.curve,
                            closed: c.closed,
                        });
                    }
                }
                Ok(SsiResult::Coincident) => faults.push(BoolFault::Coincident(id_a, id_b)),
                Ok(SsiResult::Points(_)) => faults.push(BoolFault::Tangent(id_a, id_b)),
                Ok(SsiResult::Empty) => {}
                Err(_) => faults.push(BoolFault::IntersectionFailed(id_a, id_b)),
            }
        }
    }
    (seams, faults)
}

#[cfg(test)]
mod tests {
    use super::*;
    use keel_geom::curve::Curve3;
    use keel_geom::surface::Frame3;
    use keel_math::vec::Vec3;

    /// Sphere centered at `center`, seamed with its pole on the x-axis
    /// and meridian in the world plane z = center.z (the equatorial
    /// plane through the center). For two such spheres displaced along
    /// z, the SSI latitude circle (at the midplane) never crosses
    /// either seam meridian, so imprints are crossing-free (M6a).
    fn z_sphere(center: Vec3, r: f64) -> Body {
        let mut b = Body::new();
        let frame = Frame3 {
            origin: center,
            x: Vec3::new(0., 1., 0.),
            y: Vec3::new(0., 0., 1.),
            z: Vec3::new(1., 0., 0.),
        };
        b.sphere(frame, r).unwrap();
        b
    }

    #[test]
    fn two_spheres_seam_is_one_circle() {
        // Unit spheres centered at z=0 and z=1.5 intersect in a single
        // latitude circle at z = 0.75 (by symmetry), radius
        // sqrt(1 - 0.75^2).
        let a = z_sphere(Vec3::ZERO, 1.0);
        let b = z_sphere(Vec3::new(0.0, 0.0, 1.5), 1.0);
        let (seams, faults) = seam_curves(&a, &b, 1e-9);
        assert!(faults.is_empty(), "unexpected faults: {faults:?}");
        assert_eq!(seams.len(), 1, "expected one seam circle");
        let s = &seams[0];
        assert!(s.closed);
        match &s.curve {
            Curve3::Circle(c) => {
                assert!((c.center.z - 0.75).abs() < 1e-9, "center z {}", c.center.z);
                let expected_r = (1.0f64 - 0.75 * 0.75).sqrt();
                assert!((c.radius - expected_r).abs() < 1e-9, "radius {}", c.radius);
            }
            other => panic!("expected circle, got {other:?}"),
        }
    }

    #[test]
    fn imprint_pair_splits_both_spheres() {
        let a = z_sphere(Vec3::ZERO, 1.0);
        let b = z_sphere(Vec3::new(0.0, 0.0, 1.5), 1.0);
        let (ia, ib, faults) = imprint_pair(&a, &b, 1e-7);
        assert!(faults.is_empty(), "faults: {faults:?}");
        // Each sphere face (1) splits into a cap disc + the rest (2).
        assert!(ia.body.validate().is_ok());
        assert!(ib.body.validate().is_ok());
        assert_eq!(ia.body.counts().f, 2, "sphere A faces after imprint");
        assert_eq!(ib.body.counts().f, 2, "sphere B faces after imprint");
        assert_eq!(ia.seam_edges.len(), 1);
        assert_eq!(ib.seam_edges.len(), 1);
        // The seam edge is the imprinted circle, manifold (radial 2)
        // within its own operand body.
        let ra = ia.body.edge(ia.seam_edges[0]).map(|e| e.radial.len());
        assert_eq!(ra, Some(2));
    }

    #[test]
    fn disjoint_spheres_have_no_seam() {
        let a = z_sphere(Vec3::ZERO, 1.0);
        let b = z_sphere(Vec3::new(0.0, 0.0, 5.0), 1.0);
        let (seams, faults) = seam_curves(&a, &b, 1e-9);
        assert!(seams.is_empty());
        assert!(faults.is_empty());
    }
}
