//! Setback vertex-blend corner patch (dossier 53 Q3): the
//! Charrot-Gregory convex-combination evaluator over the setback
//! hexagon, with Plowman-Charrot Gregory twists. Pre-2006 prior art
//! (Charrot-Gregory 1984, Varady-Rockwood 1995/1997); the patch is an
//! EVALUATOR, not a stored surface type: callers sample it through
//! `ForeignSurface` quads (the file-26 central split) and cache the
//! certified NURBS fits.
//!
//! Geometry: the 6-sided boundary alternates cross-section ARCS on
//! the three edge-blend cylinders with straight PROFILE segments on
//! the three planar supports. The cross-derivative field of each side
//! is FORCED by corner compatibility (D_i(0) = -B_{i-1}'(1), D_i(1) =
//! +B_{i+1}'(0), which lie in the side's host tangent plane exactly
//! because the cylinders are tangent to the supports along the
//! springs) and interpolated IN THE SIDE'S OWN SURFACE BASIS between
//! those ends, so the field stays in the host tangent plane along the
//! whole side and the patch meets each band and each support with G1
//! continuity. Rational perpendicular-distance-squared weights
//! localize each corner interpolant; the Gregory twist resolves the
//! per-corner incompatible mixed partials.

use crate::curve::Circle3;
use crate::foreign::ForeignSurface;
use keel_math::vec::Vec3;

/// One side of the setback hexagon, oriented so side `i` runs from
/// 2n-gon corner `i` to corner `i + 1` (B_i(0) = B_{i-1}(1)).
#[derive(Clone, Debug)]
pub enum HexSide {
    /// Cross-section arc on an edge-blend cylinder: the circle, the
    /// fin-ordered angle range, and the cylinder's UNIT axis (any
    /// sign; the cross field decomposes onto it).
    Arc {
        circle: Circle3,
        t0: f64,
        t1: f64,
        axis: Vec3,
    },
    /// Straight profile segment on a planar support.
    Seg { a: Vec3, b: Vec3 },
}

impl HexSide {
    fn point(&self, t: f64) -> Vec3 {
        match self {
            HexSide::Arc { circle, t0, t1, .. } => circle.point(t0 + t * (t1 - t0)),
            HexSide::Seg { a, b } => *a + (*b - *a) * t,
        }
    }
    /// dB/dt with t the side's own [0,1] parameter.
    fn tangent(&self, t: f64) -> Vec3 {
        match self {
            HexSide::Arc { circle, t0, t1, .. } => {
                let th = t0 + t * (t1 - t0);
                (circle.y_axis * th.cos() - circle.x_axis * th.sin()) * circle.radius * (t1 - t0)
            }
            HexSide::Seg { a, b } => *b - *a,
        }
    }
    /// Unit in-surface tangent direction at t (the second basis vector
    /// of an arc side; segments use the plane directly).
    fn unit_tangent(&self, t: f64) -> Vec3 {
        match self {
            HexSide::Arc { circle, t0, t1, .. } => {
                let th = t0 + t * (t1 - t0);
                (circle.y_axis * th.cos() - circle.x_axis * th.sin()) * (t1 - t0).signum()
            }
            HexSide::Seg { a, b } => (*b - *a).try_normalize().unwrap_or(Vec3::ZERO),
        }
    }
}

/// Per-side cross-derivative field, precomputed from the forced
/// corner values in the side's own surface basis.
#[derive(Clone, Debug)]
enum CrossField {
    /// (axial, tangential) coefficient pairs at the side's two ends;
    /// the basis is (axis, unit_tangent(t)), so the lerped field stays
    /// on the cylinder's tangent plane for every t.
    Arc { c0: (f64, f64), c1: (f64, f64) },
    /// End vectors lerped directly (both lie in the support plane).
    Seg { d0: Vec3, d1: Vec3 },
}

/// The Charrot-Gregory setback corner patch over a regular hexagon
/// domain (circumradius 1, vertex i at angle 60 i degrees).
#[derive(Clone, Debug)]
pub struct CornerPatch {
    sides: [HexSide; 6],
    cross: [CrossField; 6],
}

/// Domain vertex i of the regular hexagon.
pub fn hex_vertex(i: usize) -> (f64, f64) {
    let a = core::f64::consts::FRAC_PI_3 * (i % 6) as f64;
    (a.cos(), a.sin())
}

impl CornerPatch {
    /// Build the patch; the sides must be corner-compatible
    /// (consecutive endpoints coincide).
    pub fn new(sides: [HexSide; 6]) -> Result<Self, crate::GeomError> {
        for i in 0..6 {
            let p_end = sides[i].point(1.0);
            let p_next = sides[(i + 1) % 6].point(0.0);
            if (p_end - p_next).norm() > 1e-7 {
                return Err(crate::GeomError::Degenerate);
            }
        }
        // Forced cross-field ends: D_i(0) = -B_{i-1}'(1) and
        // D_i(1) = +B_{i+1}'(0), decomposed in the side's basis (the
        // decomposition drops only numerical out-of-plane fuzz; the
        // exact vectors lie in the tangent plane by spring tangency).
        let cross = core::array::from_fn(|i| {
            let prev = (i + 5) % 6;
            let next = (i + 1) % 6;
            let e0 = sides[prev].tangent(1.0) * -1.0;
            let e1 = sides[next].tangent(0.0);
            match &sides[i] {
                HexSide::Arc { axis, .. } => {
                    let th0 = sides[i].unit_tangent(0.0);
                    let th1 = sides[i].unit_tangent(1.0);
                    CrossField::Arc {
                        c0: (e0.dot(*axis), e0.dot(th0)),
                        c1: (e1.dot(*axis), e1.dot(th1)),
                    }
                }
                HexSide::Seg { .. } => CrossField::Seg { d0: e0, d1: e1 },
            }
        });
        Ok(CornerPatch { sides, cross })
    }

    /// Cross-derivative field of side i at its parameter t.
    fn cross_at(&self, i: usize, t: f64) -> Vec3 {
        match (&self.cross[i], &self.sides[i]) {
            (CrossField::Arc { c0, c1 }, HexSide::Arc { axis, .. }) => {
                let a = c0.0 + t * (c1.0 - c0.0);
                let b = c0.1 + t * (c1.1 - c0.1);
                *axis * a + self.sides[i].unit_tangent(t) * b
            }
            (CrossField::Seg { d0, d1 }, _) => *d0 + (*d1 - *d0) * t,
            _ => Vec3::ZERO,
        }
    }

    /// d/dt of the cross field (the Gregory twist inputs).
    fn cross_deriv_at(&self, i: usize, t: f64) -> Vec3 {
        match (&self.cross[i], &self.sides[i]) {
            (
                CrossField::Arc { c0, c1 },
                HexSide::Arc {
                    circle,
                    t0,
                    t1,
                    axis,
                },
            ) => {
                let b = c0.1 + t * (c1.1 - c0.1);
                let th = t0 + t * (t1 - t0);
                // d/dt of unit_tangent: rotate by the angular speed.
                let ut_d = (circle.y_axis * -th.sin() - circle.x_axis * th.cos())
                    * (t1 - t0)
                    * (t1 - t0).signum();
                *axis * (c1.0 - c0.0) + self.sides[i].unit_tangent(t) * (c1.1 - c0.1) + ut_d * b
            }
            (CrossField::Seg { d0, d1 }, _) => *d1 - *d0,
            _ => Vec3::ZERO,
        }
    }

    /// Evaluate the convex-combination patch at hexagon-domain (x, y).
    pub fn eval(&self, x: f64, y: f64) -> Vec3 {
        // Perpendicular distances to the six side lines, positive
        // toward the centre.
        let mut d = [0.0f64; 6];
        for (j, dj) in d.iter_mut().enumerate() {
            let (ax, ay) = hex_vertex(j);
            let (bx, by) = hex_vertex(j + 1);
            let (ex, ey) = (bx - ax, by - ay);
            let len = (ex * ex + ey * ey).sqrt();
            // Inward normal of a CCW polygon side: left of the edge.
            let (nx, ny) = (-ey / len, ex / len);
            *dj = ((x - ax) * nx + (y - ay) * ny).max(0.0);
        }
        // Radial-sweep side parameters s_m = d_{m-1}/(d_{m-1}+d_{m+1}).
        let s = |m: usize| -> f64 {
            let dp = d[(m + 5) % 6];
            let dn = d[(m + 1) % 6];
            if dp + dn < 1e-300 {
                0.5
            } else {
                dp / (dp + dn)
            }
        };
        let mut acc = Vec3::ZERO;
        let mut wsum = 0.0f64;
        for i in 0..6 {
            let prev = (i + 5) % 6;
            // Weight: product of squared distances to the four sides
            // NOT meeting at corner i (first-order flat on every other
            // side, the G1 localization device).
            let mut w = 1.0f64;
            for (j, &dj) in d.iter().enumerate() {
                if j != i && j != prev {
                    w *= dj * dj;
                }
            }
            if w <= 0.0 {
                // Exactly on a non-adjacent side: this corner's
                // interpolant carries no weight.
                continue;
            }
            let xi = s(i);
            let eta = 1.0 - s(prev);
            // Corner interpolant: Boolean-sum Hermite of the two sides
            // meeting at corner i, with the rational Gregory twist.
            let p = self.sides[i].point(0.0);
            let t_vec = self.sides[i].tangent(0.0);
            let u_vec = self.sides[prev].tangent(1.0) * -1.0;
            let m1 = self.cross_deriv_at(i, 0.0);
            let m2 = self.cross_deriv_at(prev, 1.0) * -1.0;
            let m_twist = if xi + eta < 1e-12 {
                Vec3::ZERO
            } else {
                (m1 * eta + m2 * xi) * (1.0 / (xi + eta))
            };
            let c = self.sides[i].point(xi)
                + self.cross_at(i, xi) * eta
                + self.sides[prev].point(1.0 - eta)
                + self.cross_at(prev, 1.0 - eta) * xi
                - (p + t_vec * xi + u_vec * eta + m_twist * (xi * eta));
            acc = acc + c * w;
            wsum += w;
        }
        if wsum <= 0.0 {
            // Degenerate query (a domain corner): both adjacent sides'
            // distances vanish; return the corner position.
            for i in 0..6 {
                let (vx, vy) = hex_vertex(i);
                if (x - vx).abs() < 1e-9 && (y - vy).abs() < 1e-9 {
                    return self.sides[i].point(0.0);
                }
            }
            return Vec3::ZERO;
        }
        acc * (1.0 / wsum)
    }
}

/// One central-split quad of the hexagon, exposed as a rectangular
/// `ForeignSurface` for the certified NURBS fit: corner i's quad is
/// (V_i, mid(S_i), centre, mid(S_{i-1})), bilinearly mapped from
/// [0,1]^2.
pub struct QuadPatch<'a> {
    pub patch: &'a CornerPatch,
    /// Domain-quad corners in bilinear order (q00, q10, q11, q01).
    pub quad: [(f64, f64); 4],
}

impl ForeignSurface for QuadPatch<'_> {
    fn domain(&self) -> ((f64, f64), (f64, f64)) {
        ((0.0, 1.0), (0.0, 1.0))
    }
    fn eval(&self, u: f64, v: f64) -> Vec3 {
        let [q00, q10, q11, q01] = self.quad;
        let x = (1.0 - u) * (1.0 - v) * q00.0
            + u * (1.0 - v) * q10.0
            + u * v * q11.0
            + (1.0 - u) * v * q01.0;
        let y = (1.0 - u) * (1.0 - v) * q00.1
            + u * (1.0 - v) * q10.1
            + u * v * q11.1
            + (1.0 - u) * v * q01.1;
        self.patch.eval(x, y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::surface::Frame3;

    /// The box-corner setback hexagon, built by hand: material
    /// [0,inf)^3, equal radius r, equal setback d. Sides cycle
    /// arc-z, profile(y=0), arc-x, profile(z=0), arc-y, profile(x=0).
    fn box_corner_patch(r: f64, d: f64) -> CornerPatch {
        let circ = |c: Vec3, x: Vec3, y: Vec3| Circle3::new(c, x, y, r).expect("circle");
        let fp = core::f64::consts::FRAC_PI_2;
        let arc_z = HexSide::Arc {
            circle: circ(
                Vec3::new(r, r, d),
                Vec3::new(-1., 0., 0.),
                Vec3::new(0., -1., 0.),
            ),
            t0: 0.0,
            t1: fp,
            axis: Vec3::new(0., 0., 1.),
        };
        let arc_x = HexSide::Arc {
            circle: circ(
                Vec3::new(d, r, r),
                Vec3::new(0., -1., 0.),
                Vec3::new(0., 0., -1.),
            ),
            t0: 0.0,
            t1: fp,
            axis: Vec3::new(1., 0., 0.),
        };
        let arc_y = HexSide::Arc {
            circle: circ(
                Vec3::new(r, d, r),
                Vec3::new(0., 0., -1.),
                Vec3::new(-1., 0., 0.),
            ),
            t0: 0.0,
            t1: fp,
            axis: Vec3::new(0., 1., 0.),
        };
        let seg = |a: Vec3, b: Vec3| HexSide::Seg { a, b };
        CornerPatch::new([
            arc_z,
            seg(Vec3::new(r, 0., d), Vec3::new(d, 0., r)),
            arc_x,
            seg(Vec3::new(d, r, 0.), Vec3::new(r, d, 0.)),
            arc_y,
            seg(Vec3::new(0., d, r), Vec3::new(0., r, d)),
        ])
        .expect("compatible hexagon")
    }

    #[test]
    fn corner_patch_reproduces_its_boundary_with_g1() {
        // Evaluator-level oracle, written FIRST: (a) the patch
        // reproduces every boundary curve exactly (the regular
        // hexagon's radial-sweep parameter is linear along each side,
        // so domain fraction f maps to curve parameter f); (b) just
        // inside each side, the patch's tangent plane agrees with the
        // host (cylinder radial / support normal) to first order.
        let (r, d) = (1.0f64, 0.6f64);
        let patch = box_corner_patch(r, d);
        let hosts: [Box<dyn Fn(Vec3) -> Vec3>; 6] = [
            Box::new(move |p: Vec3| {
                let w = p - Vec3::new(r, r, p.z);
                w.try_normalize().unwrap()
            }),
            Box::new(|_| Vec3::new(0., -1., 0.)),
            Box::new(move |p: Vec3| {
                let w = p - Vec3::new(p.x, r, r);
                w.try_normalize().unwrap()
            }),
            Box::new(|_| Vec3::new(0., 0., -1.)),
            Box::new(move |p: Vec3| {
                let w = p - Vec3::new(r, p.y, r);
                w.try_normalize().unwrap()
            }),
            Box::new(|_| Vec3::new(-1., 0., 0.)),
        ];
        let sides = &patch.sides;
        for i in 0..6 {
            let (ax, ay) = hex_vertex(i);
            let (bx, by) = hex_vertex(i + 1);
            for k in 0..=8 {
                let f = k as f64 / 8.0;
                let (px, py) = (ax + f * (bx - ax), ay + f * (by - ay));
                let want = sides[i].point(f);
                let got = patch.eval(px, py);
                assert!(
                    (got - want).norm() < 1e-9,
                    "side {i} f {f}: |{got:?} - {want:?}|"
                );
                // G1: tangent plane just inside, by central differences
                // in the domain, against the host normal at the
                // boundary point.
                if k == 0 || k == 8 {
                    continue; // skip the hexagon corners (FD stencil)
                }
                let eps = 1e-3;
                let (cx, cy) = (px * (1.0 - eps), py * (1.0 - eps));
                let h = 1e-4;
                let du = (patch.eval(cx + h, cy) - patch.eval(cx - h, cy)) * (0.5 / h);
                let dv = (patch.eval(cx, cy + h) - patch.eval(cx, cy - h)) * (0.5 / h);
                let n_patch = du.cross(dv).try_normalize().unwrap();
                let n_host = hosts[i](want);
                let dot = n_patch.dot(n_host).abs();
                assert!(
                    dot > 1.0 - 5e-5,
                    "side {i} f {f}: tangent-plane angle cos {dot}"
                );
            }
        }
        // Smoke: the centre is finite and within the corner region.
        let c = patch.eval(0.0, 0.0);
        assert!(c.is_finite() && c.x > 0.0 && c.y > 0.0 && c.z > 0.0);
        // The fitted quads certify against the evaluator.
        let quad = QuadPatch {
            patch: &patch,
            quad: [hex_vertex(0), (0.0, 0.0), (0.0, 0.0), (0.0, 0.0)],
        };
        let _ = quad; // construction smoke; the fit runs in keel-topo
        let _ = Frame3::from_z(Vec3::ZERO, Vec3::new(0., 0., 1.)).unwrap();
    }
}
