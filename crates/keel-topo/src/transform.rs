//! Rigid body transform (rotation + translation) and mirror (reflection).
//! The same isometry carries every vertex point and every analytic
//! surface/curve frame; radii and angles are invariant because the motion
//! is rigid, and pcurves (parameter space) are untouched.
//!
//! Reflection (mirror) needs NO special orientation handling: reflecting
//! every frame directly sends frame.z -> M(frame.z), so the outward normal
//! (mass_properties uses frame.z x region-solidity; tessellation uses
//! frame.z x sense) maps to M(N); and because M is orthogonal the face's
//! (u, v) coordinates are preserved, so pcurves stay valid. The frame goes
//! left-handed, but nothing relies on its chirality. Transforming NURBS
//! geometry is the one follow-up.

use crate::Body;
use crate::body::TopoError;
use crate::entity::SurfaceGeom;
use keel_geom::convert::{cone_to_nurbs, cylinder_to_nurbs, sphere_to_nurbs, torus_to_nurbs};
use keel_geom::curve::{Circle3, Curve3, Ellipse3, Line3};
use keel_geom::nurbs_curve::NurbsCurve;
use keel_geom::nurbs_surface::NurbsSurface;
use keel_geom::surface::{Cone3, Cylinder3, Frame3, Plane3, Sphere3, Surface3, Torus3};
use keel_math::mat::Mat3;
use keel_math::transform::Transform3;
use keel_math::vec::Vec3;
use keel_math::vec::Vec4;
use std::collections::HashSet;

impl Body {
    /// Apply a RIGID motion (rotation + translation) to a copy of the
    /// body. Errors if `t` is not a proper rotation -- a reflection (use
    /// [`Body::mirrored`]) or a scale/shear -- or if the body carries
    /// NURBS geometry.
    pub fn transformed(&self, t: &Transform3) -> Result<Body, TopoError> {
        match Self::isometry_kind(t)? {
            IsometryKind::Rotation => self.apply_isometry(t),
            IsometryKind::Reflection => Err(TopoError::Precondition(
                "transform: reflection -- use Body::mirrored",
            )),
        }
    }

    /// Mirror a copy of the body across the plane through `plane_point`
    /// with unit-able normal `plane_normal`. Works for analytic faces
    /// (planar and curved); NURBS-carried geometry is a follow-up.
    pub fn mirrored(&self, plane_point: Vec3, plane_normal: Vec3) -> Result<Body, TopoError> {
        let m = plane_normal
            .try_normalize()
            .ok_or(TopoError::Precondition("mirror: degenerate plane normal"))?;
        // Householder reflection R(x) = (I - 2 m m^T) x + 2 (plane_point . m) m.
        let linear = Mat3::from_cols(
            Vec3::new(1.0, 0.0, 0.0) - m * (2.0 * m.x),
            Vec3::new(0.0, 1.0, 0.0) - m * (2.0 * m.y),
            Vec3::new(0.0, 0.0, 1.0) - m * (2.0 * m.z),
        );
        let t = Transform3 {
            linear,
            translation: m * (2.0 * plane_point.dot(m)),
        };
        self.apply_isometry(&t)
    }

    /// Uniformly scale a copy of the body about `center` by `factor` > 0.
    /// Planar-faced bodies only: mass_properties and tessellation sample the
    /// scaled geometry directly, so the stored pcurves (left as-is) do not
    /// affect them. Curved surfaces (whose radii would scale) and circular/
    /// elliptic/NURBS edges are a follow-up.
    pub fn scaled(&self, center: Vec3, factor: f64) -> Result<Body, TopoError> {
        if !(factor.is_finite() && factor > 0.0) {
            return Err(TopoError::Precondition(
                "scale: factor must be finite and > 0",
            ));
        }
        let mut out = self.clone();
        let vkeys: Vec<_> = out.vertices.iter().map(|(k, _)| k).collect();
        for k in vkeys {
            if let Some(v) = out.vertices.get_mut(k) {
                v.point = center + (v.point - center) * factor;
            }
        }
        let skeys: HashSet<_> = out
            .faces
            .iter()
            .filter_map(|(_, f)| f.surface.map(|(sk, _)| sk))
            .collect();
        for k in skeys {
            let Some(SurfaceGeom::Analytic(Surface3::Plane(p))) = out.surfaces.get(k).cloned()
            else {
                return Err(TopoError::Precondition(
                    "scale: curved/NURBS surfaces are a follow-up",
                ));
            };
            let frame = Frame3 {
                origin: center + (p.frame.origin - center) * factor,
                x: p.frame.x,
                y: p.frame.y,
                z: p.frame.z,
            };
            if let Some(slot) = out.surfaces.get_mut(k) {
                *slot = SurfaceGeom::Analytic(Surface3::Plane(Plane3 { frame }));
            }
        }
        let ckeys: HashSet<_> = out
            .edges
            .iter()
            .filter_map(|(_, e)| e.curve.map(|(ck, _)| ck))
            .collect();
        for k in ckeys {
            let Some(Curve3::Line(l)) = out.curves.get(k).cloned() else {
                return Err(TopoError::Precondition(
                    "scale: curved edges are a follow-up",
                ));
            };
            let nl = Line3 {
                origin: center + (l.origin - center) * factor,
                dir: l.dir,
            };
            if let Some(slot) = out.curves.get_mut(k) {
                *slot = Curve3::Line(nl);
            }
        }
        Ok(out)
    }

    /// NON-UNIFORM scale about `center` by per-axis `factors` (> 0)
    /// (swap task 26 / 43: the op the consumer's OCCT WASM cannot do at
    /// all). PLANAR bodies are EXACT: planes and lines are closed under
    /// a positive diagonal affine map (plane normals by the
    /// INVERSE-TRANSPOSE, in-plane axes rebuilt). CURVED bodies map by
    /// the exact rational route (dossier 25 sec 22): every analytic
    /// surface has an exact NURBS form (convert.rs), and a NURBS is
    /// closed under the affine map (homogeneous control points
    /// transform linearly, weights unchanged), so a scaled cylinder
    /// becomes its exact elliptic-cylinder NURBS image, a sphere an
    /// ellipsoid, etc. Circular/elliptic EDGES map to their exact image
    /// ELLIPSE (principal axes from the conjugate semi-diameters). Mass
    /// on the resulting NURBS faces degrades gracefully (the documented
    /// M5 line); tessellation and rendering are exact.
    ///
    /// SCOPE: curved faces must be FULL surfaces of revolution (full
    /// cylinders/cones/bores, full spheres/tori). A PARTIAL curved patch
    /// (a fillet band) maps to a NURBS surface whose trimmed extent the
    /// pcurve-free NURBS tessellator cannot honor, so such bodies
    /// DECLINE rather than mis-render.
    pub fn scaled_nonuniform(&self, center: Vec3, factors: Vec3) -> Result<Body, TopoError> {
        let s = factors;
        if !(s.x.is_finite() && s.y.is_finite() && s.z.is_finite())
            || s.x <= 0.0
            || s.y <= 0.0
            || s.z <= 0.0
        {
            return Err(TopoError::Precondition(
                "scale: factors must be finite and > 0",
            ));
        }
        let map_p = |p: Vec3| {
            center
                + Vec3::new(
                    (p.x - center.x) * s.x,
                    (p.y - center.y) * s.y,
                    (p.z - center.z) * s.z,
                )
        };
        let map_v = |v: Vec3| Vec3::new(v.x * s.x, v.y * s.y, v.z * s.z);
        let map_n = |n: Vec3| Vec3::new(n.x / s.x, n.y / s.y, n.z / s.z);
        // Homogeneous control point map: p' = S p + t with
        // t = center - S center; in homogeneous (P, w) where P = w e,
        // P' = S P + w t. Weight unchanged.
        let tvec = Vec3::new(
            center.x * (1.0 - s.x),
            center.y * (1.0 - s.y),
            center.z * (1.0 - s.z),
        );
        let map_h = |h: Vec4| {
            Vec4::new(
                s.x * h.x + h.w * tvec.x,
                s.y * h.y + h.w * tvec.y,
                s.z * h.z + h.w * tvec.z,
                h.w,
            )
        };
        let map_surf_nurbs = |n: &NurbsSurface| -> Result<NurbsSurface, TopoError> {
            let ctrl: Vec<Vec4> = n.homogeneous_control().iter().map(|&h| map_h(h)).collect();
            NurbsSurface::from_homogeneous(n.kv_u().clone(), n.kv_v().clone(), ctrl)
                .map_err(|_| TopoError::Precondition("scale: nurbs surface image"))
        };

        // PRE-CHECK: every curved analytic face must be a full surface of
        // revolution (else the NURBS image would over-mesh its trim).
        // A partial patch (a fillet band) declines honestly.
        for (fk, f) in self.faces.iter() {
            let Some((sk, _)) = f.surface else { continue };
            let full = match self.surfaces.get(sk) {
                Some(SurfaceGeom::Analytic(Surface3::Sphere(_)))
                | Some(SurfaceGeom::Analytic(Surface3::Torus(_))) => {
                    self.face_covers_closed_surface(fk)
                }
                Some(SurfaceGeom::Analytic(Surface3::Cylinder(c))) => {
                    self.face_full_revolution(fk, &c.frame)
                }
                Some(SurfaceGeom::Analytic(Surface3::Cone(c))) => {
                    self.face_full_revolution(fk, &c.frame)
                }
                _ => true,
            };
            if !full {
                return Err(TopoError::Precondition(
                    "scale: partial curved patch under non-uniform scale (decline)",
                ));
            }
        }

        let mut out = self.clone();
        let vkeys: Vec<_> = out.vertices.iter().map(|(k, _)| k).collect();
        for k in vkeys {
            if let Some(v) = out.vertices.get_mut(k) {
                v.point = map_p(v.point);
            }
        }
        // SURFACES (read original from self, write image into out).
        let skeys: HashSet<_> = self
            .faces
            .iter()
            .filter_map(|(_, f)| f.surface.map(|(sk, _)| sk))
            .collect();
        for k in skeys {
            let img: SurfaceGeom = match self.surfaces.get(k) {
                Some(SurfaceGeom::Analytic(Surface3::Plane(p))) => {
                    let nz = map_n(p.frame.z)
                        .try_normalize()
                        .ok_or(TopoError::Precondition("scale: degenerate plane normal"))?;
                    let nx = {
                        let cand = map_v(p.frame.x);
                        (cand - nz * cand.dot(nz))
                            .try_normalize()
                            .ok_or(TopoError::Precondition("scale: degenerate plane frame"))?
                    };
                    SurfaceGeom::Analytic(Surface3::Plane(Plane3 {
                        frame: Frame3 {
                            origin: map_p(p.frame.origin),
                            x: nx,
                            y: nz.cross(nx),
                            z: nz,
                        },
                    }))
                }
                Some(SurfaceGeom::Analytic(Surface3::Cylinder(c))) => {
                    let (h0, h1) = self.curved_height_range(k, c.frame.origin, c.frame.z)?;
                    let nurbs = cylinder_to_nurbs(c, h0, h1)
                        .map_err(|_| TopoError::Precondition("scale: cylinder to nurbs"))?;
                    SurfaceGeom::Nurbs(map_surf_nurbs(&nurbs)?)
                }
                Some(SurfaceGeom::Analytic(Surface3::Cone(c))) => {
                    let (h0, h1) = self.curved_height_range(k, c.frame.origin, c.frame.z)?;
                    let nurbs = cone_to_nurbs(c, h0, h1)
                        .map_err(|_| TopoError::Precondition("scale: cone to nurbs"))?;
                    SurfaceGeom::Nurbs(map_surf_nurbs(&nurbs)?)
                }
                Some(SurfaceGeom::Analytic(Surface3::Sphere(sp))) => {
                    let nurbs = sphere_to_nurbs(sp)
                        .map_err(|_| TopoError::Precondition("scale: sphere to nurbs"))?;
                    SurfaceGeom::Nurbs(map_surf_nurbs(&nurbs)?)
                }
                Some(SurfaceGeom::Analytic(Surface3::Torus(t))) => {
                    let nurbs = torus_to_nurbs(t)
                        .map_err(|_| TopoError::Precondition("scale: torus to nurbs"))?;
                    SurfaceGeom::Nurbs(map_surf_nurbs(&nurbs)?)
                }
                Some(SurfaceGeom::Nurbs(n)) => SurfaceGeom::Nurbs(map_surf_nurbs(n)?),
                None => continue,
            };
            if let Some(slot) = out.surfaces.get_mut(k) {
                *slot = img;
            }
        }
        // CURVES: every edge curve, plus the PCURVES of fins on PLANAR
        // faces. A planar pcurve is a 3D curve in the plane (a cap's rim
        // circle), read by single_circle_disc / the planar mass lune
        // integrator, so it must map to the image ellipse too. Pcurves
        // on faces converted to NURBS stay untouched (the pcurve-free
        // NURBS tessellator ignores them and NURBS mass declines), so
        // mapping a UV-space pcurve by the 3D map is correctly avoided.
        let mut ckeys: HashSet<_> = self
            .edges
            .iter()
            .filter_map(|(_, e)| e.curve.map(|(ck, _)| ck))
            .collect();
        for (_, fin) in self.fins.iter() {
            let Some((pk, _)) = fin.pcurve else { continue };
            let planar = self
                .loops
                .get(fin.owner)
                .and_then(|l| self.faces.get(l.face))
                .and_then(|f| f.surface)
                .map(|(sk, _)| {
                    matches!(
                        self.surfaces.get(sk),
                        Some(SurfaceGeom::Analytic(Surface3::Plane(_)))
                    )
                })
                .unwrap_or(false);
            if planar {
                ckeys.insert(pk);
            }
        }
        for k in ckeys {
            let img: Curve3 = match self.curves.get(k) {
                Some(Curve3::Line(l)) => {
                    let dir = map_v(l.dir)
                        .try_normalize()
                        .ok_or(TopoError::Precondition("scale: degenerate edge"))?;
                    Curve3::Line(Line3 {
                        origin: map_p(l.origin),
                        dir,
                    })
                }
                Some(Curve3::Circle(ci)) => Curve3::Ellipse(image_ellipse(
                    map_p(ci.center),
                    map_v(ci.x_axis * ci.radius),
                    map_v(ci.y_axis * ci.radius),
                )?),
                Some(Curve3::Ellipse(e)) => Curve3::Ellipse(image_ellipse(
                    map_p(e.center),
                    map_v(e.x_axis * e.a),
                    map_v(e.y_axis * e.b),
                )?),
                Some(Curve3::Nurbs(n)) => {
                    let ctrl: Vec<Vec4> =
                        n.homogeneous_control().iter().map(|&h| map_h(h)).collect();
                    Curve3::Nurbs(
                        NurbsCurve::from_homogeneous(n.knot_vector().clone(), ctrl)
                            .map_err(|_| TopoError::Precondition("scale: nurbs curve image"))?,
                    )
                }
                None => continue,
            };
            if let Some(slot) = out.curves.get_mut(k) {
                *slot = img;
            }
        }
        Ok(out)
    }

    /// Axial height range [h0, h1] of the face(s) using surface key `sk`,
    /// from the boundary vertices projected on `axis` through `origin`.
    fn curved_height_range(
        &self,
        sk: crate::entity::SurfaceKey,
        origin: Vec3,
        axis: Vec3,
    ) -> Result<(f64, f64), TopoError> {
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        for (fk, f) in self.faces.iter() {
            if f.surface.map(|(s, _)| s) != Some(sk) {
                continue;
            }
            for &lk in &f.loops {
                let Some(entry) = self.loops.get(lk).and_then(|l| l.fin) else {
                    continue;
                };
                let mut cur = entry;
                while let Some(fin) = self.fins.get(cur) {
                    if let Some(p) = self
                        .fin_start_vertex(cur)
                        .and_then(|v| self.vertices.get(v))
                        .map(|v| v.point)
                    {
                        let h = (p - origin).dot(axis);
                        lo = lo.min(h);
                        hi = hi.max(h);
                    }
                    cur = fin.next;
                    if cur == entry {
                        break;
                    }
                }
            }
            let _ = fk;
        }
        if lo.is_finite() && hi - lo > 1e-9 {
            Ok((lo, hi))
        } else {
            Err(TopoError::Precondition("scale: empty curved height range"))
        }
    }

    /// Does a cylinder/cone `face` cover the FULL revolution (2 pi
    /// azimuth)? The signal that it is a bore lateral / primitive
    /// lateral / cone band (whose NURBS image grids exactly) rather than
    /// a partial fillet band. Robust to the boolean splitting rims into
    /// arcs (a drilled bore's rims become arcs at the seam/imprint
    /// vertices but the lateral still spans 2 pi), where a rim-closure
    /// test would wrongly fail.
    fn face_full_revolution(&self, face: crate::entity::FaceKey, frame: &Frame3) -> bool {
        let (lo, hi) = self.cyl_angular_span(face, frame.origin, frame.x, frame.y, frame.z);
        (hi - lo) >= core::f64::consts::TAU - 1e-6
    }

    /// Classify a transform's linear part. Errors on scale/shear (non-
    /// orthonormal), where radii would not be preserved.
    fn isometry_kind(t: &Transform3) -> Result<IsometryKind, TopoError> {
        let xx = t.apply_vector(Vec3::new(1.0, 0.0, 0.0));
        let yy = t.apply_vector(Vec3::new(0.0, 1.0, 0.0));
        let zz = t.apply_vector(Vec3::new(0.0, 0.0, 1.0));
        let orthonormal = (xx.norm() - 1.0).abs() < 1e-9
            && (yy.norm() - 1.0).abs() < 1e-9
            && (zz.norm() - 1.0).abs() < 1e-9
            && xx.dot(yy).abs() < 1e-9
            && xx.dot(zz).abs() < 1e-9
            && yy.dot(zz).abs() < 1e-9;
        if !orthonormal {
            return Err(TopoError::Precondition(
                "transform: rigid (rotation/reflection) only -- scale/shear unsupported",
            ));
        }
        Ok(if xx.cross(yy).dot(zz) >= 0.0 {
            IsometryKind::Rotation
        } else {
            IsometryKind::Reflection
        })
    }

    /// Shared isometry application: carry every vertex point and every
    /// referenced analytic surface/curve frame by `t`. Works identically
    /// for rotations and reflections (see the module note).
    fn apply_isometry(&self, t: &Transform3) -> Result<Body, TopoError> {
        let mut out = self.clone();
        let fr = |f: &Frame3| Frame3 {
            origin: t.apply_point(f.origin),
            x: t.apply_vector(f.x),
            y: t.apply_vector(f.y),
            z: t.apply_vector(f.z),
        };
        // Carry a homogeneous NURBS control point (x,y,z,w) under the rigid
        // isometry: the linear part acts on (x,y,z) and w*translation is added,
        // the weight w is unchanged. A rigid map is affine, and NURBS are
        // affine-covariant, so the image spline is EXACT (no approximation) ->
        // transforming/mirroring NURBS geometry is WRONG-safe.
        let tvec = t.apply_point(Vec3::ZERO);
        let map_h = |h: Vec4| -> Vec4 {
            let r = t.apply_vector(Vec3::new(h.x, h.y, h.z));
            Vec4::new(r.x + h.w * tvec.x, r.y + h.w * tvec.y, r.z + h.w * tvec.z, h.w)
        };

        let vkeys: Vec<_> = out.vertices.iter().map(|(k, _)| k).collect();
        for k in vkeys {
            if let Some(v) = out.vertices.get_mut(k) {
                v.point = t.apply_point(v.point);
            }
        }

        // Only geometry REFERENCED by the topology -- surfaces by faces,
        // curves by edges. Pcurves (fin parameter-space curves, in the same
        // arena) and orphan geometry are deliberately left alone.
        let skeys: HashSet<_> = out
            .faces
            .iter()
            .filter_map(|(_, f)| f.surface.map(|(sk, _)| sk))
            .collect();
        for k in skeys {
            let Some(s) = out.surfaces.get(k).cloned() else {
                continue;
            };
            let img: SurfaceGeom = match s {
                SurfaceGeom::Analytic(a) => SurfaceGeom::Analytic(match a {
                    Surface3::Plane(p) => Surface3::Plane(Plane3 {
                        frame: fr(&p.frame),
                    }),
                    Surface3::Cylinder(c) => Surface3::Cylinder(Cylinder3 {
                        frame: fr(&c.frame),
                        radius: c.radius,
                    }),
                    Surface3::Cone(c) => Surface3::Cone(Cone3 {
                        frame: fr(&c.frame),
                        radius: c.radius,
                        half_angle: c.half_angle,
                    }),
                    Surface3::Sphere(c) => Surface3::Sphere(Sphere3 {
                        frame: fr(&c.frame),
                        radius: c.radius,
                    }),
                    Surface3::Torus(c) => Surface3::Torus(Torus3 {
                        frame: fr(&c.frame),
                        major: c.major,
                        minor: c.minor,
                    }),
                }),
                // NURBS surface: carry the control net by the isometry (exact).
                SurfaceGeom::Nurbs(n) => {
                    let ctrl: Vec<Vec4> =
                        n.homogeneous_control().iter().map(|&h| map_h(h)).collect();
                    SurfaceGeom::Nurbs(
                        NurbsSurface::from_homogeneous(n.kv_u().clone(), n.kv_v().clone(), ctrl)
                            .map_err(|_| {
                                TopoError::Precondition("transform: nurbs surface image")
                            })?,
                    )
                }
            };
            if let Some(slot) = out.surfaces.get_mut(k) {
                *slot = img;
            }
        }

        let ckeys: HashSet<_> = out
            .edges
            .iter()
            .filter_map(|(_, e)| e.curve.map(|(ck, _)| ck))
            .collect();
        for k in ckeys {
            let Some(c) = out.curves.get(k).cloned() else {
                continue;
            };
            let nc = match c {
                Curve3::Line(l) => Curve3::Line(Line3 {
                    origin: t.apply_point(l.origin),
                    dir: t.apply_vector(l.dir),
                }),
                Curve3::Circle(ci) => Curve3::Circle(Circle3 {
                    center: t.apply_point(ci.center),
                    x_axis: t.apply_vector(ci.x_axis),
                    y_axis: t.apply_vector(ci.y_axis),
                    radius: ci.radius,
                }),
                Curve3::Ellipse(e) => Curve3::Ellipse(Ellipse3 {
                    center: t.apply_point(e.center),
                    x_axis: t.apply_vector(e.x_axis),
                    y_axis: t.apply_vector(e.y_axis),
                    a: e.a,
                    b: e.b,
                }),
                // NURBS curve: carry the control polygon by the isometry (exact).
                Curve3::Nurbs(n) => {
                    let ctrl: Vec<Vec4> =
                        n.homogeneous_control().iter().map(|&h| map_h(h)).collect();
                    Curve3::Nurbs(
                        NurbsCurve::from_homogeneous(n.knot_vector().clone(), ctrl)
                            .map_err(|_| TopoError::Precondition("transform: nurbs curve image"))?,
                    )
                }
            };
            if let Some(slot) = out.curves.get_mut(k) {
                *slot = nc;
            }
        }
        Ok(out)
    }
}

enum IsometryKind {
    Rotation,
    Reflection,
}

/// The image ELLIPSE of a circle/ellipse under a diagonal affine map,
/// from the two CONJUGATE semi-diameter vectors `u`, `v` (the images of
/// the source's x_axis*semi and y_axis*semi). The image points are
/// P(t) = center + u cos t + v sin t; its principal axes are the
/// eigenvectors of the 2x2 metric G = [[u.u, u.v], [u.v, v.v]] and the
/// squared semi-axes are G's eigenvalues (closed-form for 2x2).
fn image_ellipse(center: Vec3, u: Vec3, v: Vec3) -> Result<Ellipse3, TopoError> {
    let bad = || TopoError::Precondition("scale: degenerate image ellipse");
    let (guu, guv, gvv) = (u.dot(u), u.dot(v), v.dot(v));
    let tr = guu + gvv;
    let det = guu * gvv - guv * guv;
    let disc = (tr * tr - 4.0 * det).max(0.0).sqrt();
    let l1 = 0.5 * (tr + disc); // larger eigenvalue (semi-major^2)
    let l2 = 0.5 * (tr - disc);
    if !(l1 > 0.0 && l2 > 0.0) {
        return Err(bad());
    }
    // Eigenvector (c, s) of G for l1: (guu - l1) c + guv s = 0.
    let (c1, s1) = if guv.abs() > 1e-14 {
        (guv, l1 - guu)
    } else if guu >= gvv {
        (1.0, 0.0)
    } else {
        (0.0, 1.0)
    };
    let ax1 = (u * c1 + v * s1).try_normalize().ok_or_else(bad)?;
    // The other principal axis is orthogonal to ax1 in the ellipse
    // plane (normal n = u x v).
    let n = u.cross(v).try_normalize().ok_or_else(bad)?;
    let ax2 = n.cross(ax1).try_normalize().ok_or_else(bad)?;
    Ellipse3::new(center, ax1, ax2, l1.sqrt(), l2.sqrt()).map_err(|_| bad())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transform_box_rigid_preserves_volume() {
        // Rotate a 2^3 block a quarter turn about z, then translate +5x.
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        let t = Transform3::from_rotation(Vec3::new(0.0, 0.0, 1.0), core::f64::consts::FRAC_PI_2)
            .unwrap()
            .then(Transform3::from_translation(Vec3::new(5.0, 0.0, 0.0)));
        let m = b.transformed(&t).unwrap();
        assert!(m.validate().is_ok(), "transformed box invalid");
        let v = m.mass_properties().unwrap().volume;
        assert!(
            (v - 8.0).abs() < 1e-9,
            "rigid transform changed volume: {v}"
        );
        // [0,2]^3 -> (x,y,z)->(-y,x,z) -> +5x: x in [3,5], y in [0,2], z in [0,2].
        let bb = m.bounding_box();
        assert!(
            (bb.min - Vec3::new(3.0, 0.0, 0.0)).norm() < 1e-9,
            "bbox min {:?}",
            bb.min
        );
        assert!(
            (bb.max - Vec3::new(5.0, 2.0, 2.0)).norm() < 1e-9,
            "bbox max {:?}",
            bb.max
        );
    }

    #[test]
    fn transform_rejects_scale_and_reflection() {
        let mut b = Body::new();
        b.block(Vec3::ZERO, 1.0, 1.0, 1.0).unwrap();
        assert!(
            b.transformed(&Transform3::from_uniform_scale(2.0)).is_err(),
            "scale should reject"
        );
        let reflect = Transform3 {
            linear: Mat3::from_cols(
                Vec3::new(-1.0, 0.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(0.0, 0.0, 1.0),
            ),
            translation: Vec3::ZERO,
        };
        assert!(
            b.transformed(&reflect).is_err(),
            "transformed must reject a reflection (mirror has its own entry)"
        );
    }

    #[test]
    fn mirror_box_across_plane() {
        // Reflect a 2^3 block across x = 0: [0,2]^3 -> [-2,0]x[0,2]x[0,2],
        // volume invariant, a valid (re-oriented) solid.
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        let m = b.mirrored(Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0)).unwrap();
        assert!(m.validate().is_ok(), "mirrored box invalid");
        let v = m.mass_properties().unwrap().volume;
        assert!((v - 8.0).abs() < 1e-9, "mirror changed volume: {v}");
        let bb = m.bounding_box();
        assert!(
            (bb.min - Vec3::new(-2.0, 0.0, 0.0)).norm() < 1e-9,
            "bbox min {:?}",
            bb.min
        );
        assert!(
            (bb.max - Vec3::new(0.0, 2.0, 2.0)).norm() < 1e-9,
            "bbox max {:?}",
            bb.max
        );
    }

    #[test]
    fn mirror_curved_body() {
        // An offset cylinder (axis through x = 2, radius 1, height 2)
        // mirrored across x = 0 -> axis through x = -2; direct frame
        // reflection handles the curved face with no special casing.
        let frame = Frame3::from_z(Vec3::new(2.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.0)).unwrap();
        let mut b = Body::new();
        b.cylinder(frame, 1.0, 2.0).unwrap();
        let m = b.mirrored(Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0)).unwrap();
        assert!(m.validate().is_ok(), "mirrored cylinder invalid");
        let v = m.mesh_volume();
        let expect = 2.0 * core::f64::consts::PI; // pi r^2 h = pi*1*2
        assert!(
            (v - expect).abs() < expect * 0.02,
            "mirrored cylinder volume {v} != ~{expect}"
        );
        let bb = m.bounding_box();
        assert!(
            (bb.min - Vec3::new(-3.0, -1.0, 0.0)).norm() < 1e-9
                && (bb.max - Vec3::new(-1.0, 1.0, 2.0)).norm() < 1e-9,
            "mirrored cylinder bbox [{:?},{:?}]",
            bb.min,
            bb.max
        );
    }

    #[test]
    fn transform_cylinder_preserves_analytic_volume() {
        // Rotating a CURVED body must keep analytic mass_properties exact:
        // pcurves are parameter-space (untouched), and the rotated surface
        // frame maps them to the rotated 3D surface -- so the integral is
        // the rotated original. (Coverage: transform on curved bodies was
        // only checked via validate+mesh_volume before.)
        let frame = Frame3::from_z(Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0)).unwrap();
        let mut b = Body::new();
        b.cylinder(frame, 1.0, 2.0).unwrap();
        let v0 = b.mass_properties().unwrap().volume; // pi r^2 h = 2pi
        let t = Transform3::from_rotation(Vec3::new(1.0, 0.0, 0.0), 0.7).unwrap();
        let m = b.transformed(&t).unwrap();
        assert!(m.validate().is_ok(), "transformed cylinder invalid");
        let v1 = m.mass_properties().unwrap().volume;
        assert!(
            (v1 - v0).abs() < 1e-9,
            "rotation changed cylinder analytic volume: {v0} -> {v1}"
        );
    }

    #[test]
    fn scale_box_cubes_the_volume() {
        // [0,2]^3 (volume 8) scaled 2x about the origin -> [0,4]^3 (volume
        // 64 = 8 * 2^3), a valid solid.
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        let s = b.scaled(Vec3::ZERO, 2.0).unwrap();
        assert!(s.validate().is_ok(), "scaled box invalid");
        let v = s.mass_properties().unwrap().volume;
        assert!((v - 64.0).abs() < 1e-9, "scale^3 volume {v} != 64");
        let bb = s.bounding_box();
        assert!(
            (bb.min - Vec3::ZERO).norm() < 1e-9
                && (bb.max - Vec3::new(4.0, 4.0, 4.0)).norm() < 1e-9,
            "scaled bbox [{:?},{:?}]",
            bb.min,
            bb.max
        );
    }

    #[test]
    fn nonuniform_scale_exact_volumes() {
        // Axis-aligned: [0,2]^3 scaled (2,1,3) -> volume 8*6 = 48,
        // bbox [0,4]x[0,2]x[0,6], exact, valid, mass == mesh.
        let mut b = Body::new();
        b.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        let s = b
            .scaled_nonuniform(Vec3::ZERO, Vec3::new(2.0, 1.0, 3.0))
            .unwrap();
        assert!(s.validate().is_ok(), "nonuniform scaled box invalid");
        let v = s.mass_properties().unwrap().volume;
        assert!((v - 48.0).abs() < 1e-9 * 49.0, "volume {v} != 48");
        let mesh = s.mesh_volume();
        assert!(
            (mesh - v).abs() < 1e-9 * (1.0 + v),
            "mass {v} != mesh {mesh}"
        );
        let bb = s.bounding_box();
        assert!(
            (bb.max - Vec3::new(4.0, 2.0, 6.0)).norm() < 1e-9,
            "bbox max {:?}",
            bb.max
        );

        // ROTATED box: normals off-axis exercise the inverse-transpose.
        // Volume still multiplies by det = 6 exactly.
        let mut b2 = Body::new();
        b2.block(Vec3::ZERO, 2.0, 2.0, 2.0).unwrap();
        let t = Transform3::from_rotation(Vec3::new(1.0, 1.0, 0.3).try_normalize().unwrap(), 0.6)
            .unwrap();
        let r = b2.transformed(&t).unwrap();
        let s2 = r
            .scaled_nonuniform(Vec3::new(0.5, -1.0, 2.0), Vec3::new(2.0, 1.0, 3.0))
            .unwrap();
        assert!(s2.validate().is_ok(), "rotated nonuniform scale invalid");
        let v2 = s2.mass_properties().unwrap().volume;
        assert!(
            (v2 - 48.0).abs() < 1e-9 * 49.0,
            "rotated nonuniform volume {v2} != 48"
        );
        let mesh2 = s2.mesh_volume();
        assert!(
            (mesh2 - v2).abs() < 1e-9 * (1.0 + v2),
            "rotated: mass {v2} != mesh {mesh2}"
        );

        // Curved bodies map by the NURBS-image route (task 43): a
        // cylinder becomes an exact elliptic cylinder.
        let frame = Frame3::from_z(Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0)).unwrap();
        let mut cyl = Body::new();
        cyl.cylinder(frame, 1.0, 2.0).unwrap();
        let ec = cyl
            .scaled_nonuniform(Vec3::new(0.0, 0.0, 1.0), Vec3::new(2.0, 1.0, 1.0))
            .unwrap();
        assert!(ec.validate().is_ok(), "scaled cylinder invalid");
    }

    #[test]
    fn scale_cylinder_to_elliptic_cylinder_exact_volume() {
        // r=1, h=2 cylinder scaled (2, 1, 1) about its centre -> an
        // elliptic cylinder, semi-axes (2, 1), height 2: volume pi a b h
        // = 4 pi. Mesh (chordal band) is the oracle (NURBS mass declines).
        let pi = core::f64::consts::PI;
        let frame = Frame3::from_z(Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0)).unwrap();
        let mut cyl = Body::new();
        cyl.cylinder(frame, 1.0, 2.0).unwrap();
        let ec = cyl
            .scaled_nonuniform(Vec3::new(0.0, 0.0, 1.0), Vec3::new(2.0, 1.0, 1.0))
            .unwrap();
        assert!(ec.validate().is_ok(), "elliptic cylinder invalid");
        let v = ec.mesh_volume();
        let exact = pi * 2.0 * 1.0 * 2.0;
        assert!(
            (v - exact).abs() <= 2e-2 * (1.0 + exact),
            "elliptic cylinder volume {v} vs exact {exact}"
        );
        // The bore demo: a drilled plate whose round hole becomes an
        // ellipse. Block 4x4x1, bore r=1 at centre; scale x by 2.
        let mut plate = Body::new();
        plate.block(Vec3::ZERO, 4.0, 4.0, 1.0).unwrap();
        let bf = Frame3::from_z(Vec3::new(2.0, 2.0, -0.5), Vec3::new(0.0, 0.0, 1.0)).unwrap();
        let mut tool = Body::new();
        tool.cylinder(bf, 1.0, 2.0).unwrap();
        let drilled =
            crate::boolean::boolean(&plate, &tool, crate::boolean::BoolOp::Difference, 1e-7)
                .unwrap()
                .body;
        let scaled = drilled
            .scaled_nonuniform(Vec3::new(2.0, 2.0, 0.5), Vec3::new(2.0, 1.0, 1.0))
            .unwrap();
        assert!(scaled.validate().is_ok(), "scaled drilled plate invalid");
        // Block 8x4x1 minus an elliptic bore semi-axes (2,1): volume
        // 32 - pi*2*1*1 = 32 - 2pi.
        let v = scaled.mesh_volume();
        let exact = 32.0 - pi * 2.0 * 1.0 * 1.0;
        assert!(
            (v - exact).abs() <= 2e-2 * (1.0 + exact),
            "scaled bore volume {v} vs exact {exact}"
        );
    }

    #[test]
    fn scale_sphere_to_ellipsoid_exact_volume() {
        let pi = core::f64::consts::PI;
        let frame = Frame3::from_z(Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0)).unwrap();
        let mut s = Body::new();
        s.sphere(frame, 1.0).unwrap();
        let ell = s
            .scaled_nonuniform(Vec3::ZERO, Vec3::new(2.0, 1.5, 1.0))
            .unwrap();
        assert!(ell.validate().is_ok(), "ellipsoid invalid");
        let v = ell.mesh_volume();
        let exact = 4.0 / 3.0 * pi * 2.0 * 1.5 * 1.0; // (4/3) pi a b c
        assert!(
            (v - exact).abs() <= 3e-2 * (1.0 + exact),
            "ellipsoid volume {v} vs exact {exact}"
        );
    }

    #[test]
    fn scale_declines_partial_curved_patch() {
        // A filleted block: the fillet is a PARTIAL cylinder band, which
        // the NURBS-image tessellator cannot trim, so non-uniform scale
        // declines (honestly) rather than mis-render.
        use crate::entity::{AnyKey, EdgeKey};
        let mut b = Body::new();
        b.block(Vec3::ZERO, 4.0, 1.0, 2.0).unwrap();
        let e: EdgeKey = b
            .entity_ids()
            .filter_map(|id| match b.lookup(id) {
                Some(AnyKey::Edge(k)) => Some(k),
                _ => None,
            })
            .find(|&k| {
                let Some(edge) = b.edge(k) else { return false };
                let (Some(p0), Some(p1)) = (
                    b.vertex(edge.bounds.0).map(|v| v.point),
                    b.vertex(edge.bounds.1).map(|v| v.point),
                ) else {
                    return false;
                };
                ((p0 + p1) * 0.5 - Vec3::new(2.0, 0.0, 2.0)).norm() < 1e-9
            })
            .expect("wall-top edge");
        let filleted = b.fillet_edge(e, 0.3).unwrap();
        assert!(
            filleted
                .scaled_nonuniform(Vec3::ZERO, Vec3::new(2.0, 1.0, 1.0))
                .is_err(),
            "partial fillet band under non-uniform scale must decline"
        );
    }

    #[test]
    fn scale_rejects_curved_and_bad_factor() {
        let frame = Frame3::from_z(Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0)).unwrap();
        let mut cyl = Body::new();
        cyl.cylinder(frame, 1.0, 2.0).unwrap();
        assert!(
            cyl.scaled(Vec3::ZERO, 2.0).is_err(),
            "curved body scale is a follow-up"
        );
        let mut bx = Body::new();
        bx.block(Vec3::ZERO, 1.0, 1.0, 1.0).unwrap();
        assert!(bx.scaled(Vec3::ZERO, 0.0).is_err(), "zero factor rejects");
    }
}
