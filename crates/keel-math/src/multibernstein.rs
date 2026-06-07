//! Multivariate polynomials in tensor-product Bernstein form on
//! [0,1]^n and the Projected Polyhedron global root solver
//! (Sherbrooke and Patrikalakis 1993; spec D6). Every step is a
//! conservative convex-hull exclusion, so no root is ever lost.
//!
//! Known limitation: a polynomial identically zero on a sub-box
//! (continuum solution sets, tangential contact along a curve) cannot
//! contract; the node budget exhausts and `solve_system` returns
//! None, the deliberate signal to switch to a dedicated coincidence
//! handler (M5 work).

/// Tensor-product Bernstein polynomial. Coefficients row-major with
/// the LAST variable contiguous.
#[derive(Clone, Debug, PartialEq)]
pub struct MultiBernstein {
    degrees: Vec<usize>,
    coeffs: Vec<f64>,
}

/// A root enclosure box in [0,1]^n.
#[derive(Clone, Debug)]
pub struct RootBox {
    pub lo: Vec<f64>,
    pub hi: Vec<f64>,
}

impl MultiBernstein {
    /// None for empty/mismatched/non-finite input.
    pub fn new(degrees: Vec<usize>, coeffs: Vec<f64>) -> Option<Self> {
        if degrees.is_empty() {
            return None;
        }
        let n: usize = degrees.iter().map(|d| d + 1).product();
        if coeffs.len() != n || coeffs.iter().any(|c| !c.is_finite()) {
            return None;
        }
        Some(Self { degrees, coeffs })
    }

    /// Exact power-of-two coefficient canonicalization: roots depend
    /// only on signs and ratios (M1 fuzz lesson: scale pathologies
    /// overflow). Applied at solver entry, never in `new`, so `eval`
    /// returns true values.
    fn normalized(&self) -> Self {
        let m = self.coeffs.iter().fold(0.0f64, |acc, c| acc.max(c.abs()));
        if m == 0.0 {
            return self.clone();
        }
        let e = m.log2().ceil() as i32;
        let h = e / 2;
        let (s1, s2) = (2.0f64.powi(-h), 2.0f64.powi(-(e - h)));
        Self {
            degrees: self.degrees.clone(),
            coeffs: self.coeffs.iter().map(|c| c * s1 * s2).collect(),
        }
    }

    pub fn vars(&self) -> usize {
        self.degrees.len()
    }

    pub fn degrees(&self) -> &[usize] {
        &self.degrees
    }

    fn stride(&self, axis: usize) -> usize {
        self.degrees[axis + 1..].iter().map(|d| d + 1).product()
    }

    /// De Casteljau evaluation, collapsing the last (contiguous) axis
    /// first.
    pub fn eval(&self, x: &[f64]) -> f64 {
        debug_assert_eq!(x.len(), self.vars());
        let mut w = self.coeffs.clone();
        for ax in (0..self.vars()).rev() {
            let t = x[ax];
            let d = self.degrees[ax];
            let blocks = w.len() / (d + 1);
            let mut out = vec![0.0; blocks];
            for (b, slot) in out.iter_mut().enumerate() {
                let seg = &mut w[b * (d + 1)..(b + 1) * (d + 1)];
                let mut len = d + 1;
                while len > 1 {
                    for i in 0..len - 1 {
                        seg[i] = (1.0 - t) * seg[i] + t * seg[i + 1];
                    }
                    len -= 1;
                }
                *slot = seg[0];
            }
            w = out;
        }
        w[0]
    }

    /// Split along `axis` at local parameter t (de Casteljau lanes).
    pub fn subdivide(&self, axis: usize, t: f64) -> (Self, Self) {
        let d = self.degrees[axis];
        let stride = self.stride(axis);
        let lane_span = (d + 1) * stride;
        let outer = self.coeffs.len() / lane_span;
        let mut left = self.coeffs.clone();
        let mut right = self.coeffs.clone();
        let mut lane = vec![0.0; d + 1];
        for o in 0..outer {
            for s in 0..stride {
                let base = o * lane_span + s;
                for (i, l) in lane.iter_mut().enumerate() {
                    *l = self.coeffs[base + i * stride];
                }
                left[base] = lane[0];
                right[base + d * stride] = lane[d];
                for level in 1..=d {
                    for i in 0..=(d - level) {
                        lane[i] = (1.0 - t) * lane[i] + t * lane[i + 1];
                    }
                    left[base + level * stride] = lane[0];
                    right[base + (d - level) * stride] = lane[d - level];
                }
            }
        }
        (
            Self {
                degrees: self.degrees.clone(),
                coeffs: left,
            },
            Self {
                degrees: self.degrees.clone(),
                coeffs: right,
            },
        )
    }

    /// Per-level (min, max) coefficient envelope along `axis`.
    fn envelope(&self, axis: usize) -> Vec<(f64, f64)> {
        let d = self.degrees[axis];
        let stride = self.stride(axis);
        let mut env = vec![(f64::INFINITY, f64::NEG_INFINITY); d + 1];
        for (idx, &c) in self.coeffs.iter().enumerate() {
            let i = (idx / stride) % (d + 1);
            env[i].0 = env[i].0.min(c);
            env[i].1 = env[i].1.max(c);
        }
        env
    }
}

/// The x-span of [0,1] where the convex hull of the projected control
/// points {(i/d, c)} can cross zero: where the lower convex chain of
/// the minima is <= 0 AND the upper concave chain of the maxima is
/// >= 0. None when the hull misses the axis (no root possible).
fn zero_interval(env: &[(f64, f64)]) -> Option<(f64, f64)> {
    let d = env.len() - 1;
    if d == 0 {
        return if env[0].0 <= 0.0 && env[0].1 >= 0.0 {
            Some((0.0, 1.0))
        } else {
            None
        };
    }
    let xs: Vec<f64> = (0..=d).map(|i| i as f64 / d as f64).collect();
    let lower = chain(&xs, env, |e| e.0, true);
    let upper = chain(&xs, env, |e| e.1, false);
    let a = below_zero_span(&lower)?;
    let b = below_zero_span(&negate(&upper))?;
    let lo = a.0.max(b.0);
    let hi = a.1.min(b.1);
    if lo <= hi { Some((lo, hi)) } else { None }
}

/// Monotone-chain hull over points already sorted in x. `lower` keeps
/// the convex-from-below chain; the flag flips for the concave upper
/// chain.
fn chain(
    xs: &[f64],
    env: &[(f64, f64)],
    pick: impl Fn(&(f64, f64)) -> f64,
    lower: bool,
) -> Vec<(f64, f64)> {
    let mut out: Vec<(f64, f64)> = Vec::with_capacity(xs.len());
    for (i, &x) in xs.iter().enumerate() {
        let y = pick(&env[i]);
        while out.len() >= 2 {
            let (x1, y1) = out[out.len() - 2];
            let (x2, y2) = out[out.len() - 1];
            // cross(out[-2], out[-1], new) > 0 is a counterclockwise
            // turn: convex from below. Lower hull keeps those; the
            // concave upper hull keeps clockwise turns.
            let cross = (x2 - x1) * (y - y1) - (y2 - y1) * (x - x1);
            let keep = if lower { cross > 0.0 } else { cross < 0.0 };
            if keep {
                break;
            }
            out.pop();
        }
        out.push((x, y));
    }
    out
}

fn negate(c: &[(f64, f64)]) -> Vec<(f64, f64)> {
    c.iter().map(|&(x, y)| (x, -y)).collect()
}

/// For a CONVEX piecewise-linear chain, the x-span where y <= 0
/// (convexity makes it a single interval). None when always positive.
fn below_zero_span(chain: &[(f64, f64)]) -> Option<(f64, f64)> {
    if chain.len() == 1 {
        return if chain[0].1 <= 0.0 {
            Some((0.0, 1.0))
        } else {
            None
        };
    }
    let mut lo: Option<f64> = None;
    let mut hi: Option<f64> = None;
    for w in chain.windows(2) {
        let ((x1, y1), (x2, y2)) = (w[0], w[1]);
        if y1 <= 0.0 {
            lo.get_or_insert(x1);
            hi = Some(if y2 <= 0.0 {
                x2
            } else {
                x1 + (x2 - x1) * (y1 / (y1 - y2))
            });
        } else if y2 <= 0.0 {
            let xc = x1 + (x2 - x1) * (y1 / (y1 - y2));
            lo.get_or_insert(xc);
            hi = Some(x2);
        }
    }
    match (lo, hi) {
        (Some(a), Some(b)) => Some((a, b)),
        _ => None,
    }
}

/// Projected Polyhedron solve over [0,1]^n. Returns None when the
/// node budget is exhausted.
pub fn solve_system(
    polys: &[MultiBernstein],
    tol: f64,
    max_nodes: usize,
) -> Option<Vec<RootBox>> {
    if polys.is_empty() {
        return Some(Vec::new());
    }
    let n = polys[0].vars();
    debug_assert!(polys.iter().all(|p| p.vars() == n));
    let mut out: Vec<RootBox> = Vec::new();
    let normalized: Vec<MultiBernstein> = polys.iter().map(|p| p.normalized()).collect();
    let mut stack = vec![(normalized, vec![(0.0f64, 1.0f64); n])];
    let mut nodes = 0usize;
    while let Some((mut ps, mut bx)) = stack.pop() {
        nodes += 1;
        if nodes > max_nodes {
            return None;
        }
        // Contract every axis by the hull projection of every poly.
        let mut alive = true;
        let mut worst_width = 0.0f64;
        for (ax, slot) in bx.iter_mut().enumerate() {
            let mut lo = 0.0f64;
            let mut hi = 1.0f64;
            for p in &ps {
                match zero_interval(&p.envelope(ax)) {
                    None => {
                        alive = false;
                        break;
                    }
                    Some((a, b)) => {
                        lo = lo.max(a);
                        hi = hi.min(b);
                    }
                }
            }
            if !alive || lo > hi {
                alive = false;
                break;
            }
            // Guard band: inflate the span slightly before cropping.
            // Floating-point hulls of deeply subdivided coefficients
            // are not exactly conservative; cropping exactly at the
            // computed crossing can shave a true root off the box.
            let band = 0.01 * (hi - lo) + 1e-12;
            let lo = (lo - band).max(0.0);
            let hi = (hi + band).min(1.0);
            worst_width = worst_width.max(hi - lo);
            // Crop the polynomials and the global box to [lo, hi].
            if lo > 0.0 || hi < 1.0 {
                ps = ps
                    .iter()
                    .map(|p| {
                        let (_, r) = p.subdivide(ax, lo);
                        let t = if 1.0 - lo > 0.0 {
                            ((hi - lo) / (1.0 - lo)).clamp(0.0, 1.0)
                        } else {
                            0.0
                        };
                        let (l, _) = r.subdivide(ax, t);
                        l
                    })
                    .collect();
                let w = slot.1 - slot.0;
                *slot = (slot.0 + lo * w, slot.0 + hi * w);
            }
        }
        if !alive {
            // An exclusion firing on a box already at tolerance scale
            // is the f64 noise floor, not a proof of emptiness: the
            // box only got this small by contracting onto a (near-)
            // root. Emit conservatively; consumers polish and verify
            // (the certified IPP variant is M5 work).
            if bx.iter().all(|(a, b)| b - a <= 8.0 * tol) {
                out.push(RootBox {
                    lo: bx.iter().map(|x| x.0).collect(),
                    hi: bx.iter().map(|x| x.1).collect(),
                });
            }
            continue;
        }
        if bx.iter().all(|(a, b)| b - a <= tol) {
            out.push(RootBox {
                lo: bx.iter().map(|x| x.0).collect(),
                hi: bx.iter().map(|x| x.1).collect(),
            });
            continue;
        }
        if worst_width < 0.8 {
            // Good contraction everywhere: iterate without splitting.
            stack.push((ps, bx));
            continue;
        }
        // Stalled: bisect the axis widest in ORIGINAL coordinates.
        let ax = (0..n)
            .max_by(|&i, &j| (bx[i].1 - bx[i].0).total_cmp(&(bx[j].1 - bx[j].0)))
            .unwrap_or(0);
        let subs: Vec<(MultiBernstein, MultiBernstein)> =
            ps.iter().map(|p| p.subdivide(ax, 0.5)).collect();
        let (lo_ax, hi_ax) = bx[ax];
        let mid = 0.5 * lo_ax + 0.5 * hi_ax; // overflow-safe midpoint
        let mut bl = bx.clone();
        bl[ax] = (lo_ax, mid);
        let mut br = bx;
        br[ax] = (mid, hi_ax);
        stack.push((subs.iter().map(|s| s.0.clone()).collect(), bl));
        stack.push((subs.iter().map(|s| s.1.clone()).collect(), br));
    }
    // Merge boxes that touch (a root cluster cut by a bisection plane
    // is one root, not two; noise-floor emissions hug their root box).
    merge_touching(&mut out, 8.0 * tol);
    Some(out)
}

/// Merge root boxes that touch within 2 * tol per axis.
fn merge_touching(boxes: &mut Vec<RootBox>, tol: f64) {
    let mut i = 0;
    while i < boxes.len() {
        let mut j = i + 1;
        let mut merged = false;
        while j < boxes.len() {
            let touch = (0..boxes[i].lo.len()).all(|k| {
                boxes[i].lo[k] <= boxes[j].hi[k] + 2.0 * tol
                    && boxes[j].lo[k] <= boxes[i].hi[k] + 2.0 * tol
            });
            if touch {
                for k in 0..boxes[i].lo.len() {
                    boxes[i].lo[k] = boxes[i].lo[k].min(boxes[j].lo[k]);
                    boxes[i].hi[k] = boxes[i].hi[k].max(boxes[j].hi[k]);
                }
                boxes.remove(j);
                merged = true;
            } else {
                j += 1;
            }
        }
        if merged {
            // Re-scan: the grown box may now touch earlier boxes.
            i = 0;
        } else {
            i += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn eval_matches_tensor_construction() {
        // f(x, y) = (x^2 - 0.25) + (y^2 - 0.25): coefficients are the
        // outer sum of the univariate Bernstein vectors.
        let u = [-0.25, -0.25, 0.75];
        let mut coeffs = Vec::new();
        for a in u {
            for b in u {
                coeffs.push(a + b);
            }
        }
        let f = MultiBernstein::new(vec![2, 2], coeffs).unwrap();
        for &(x, y) in &[(0.0, 0.0), (0.5, 0.5), (0.3, 0.8), (1.0, 0.2)] {
            let want = (x * x - 0.25) + (y * y - 0.25);
            assert!((f.eval(&[x, y]) - want).abs() < 1e-14, "at {x} {y}");
        }
    }

    #[test]
    fn subdivision_preserves_values() {
        let f = MultiBernstein::new(vec![2, 1], vec![1., -2., 0.5, 3., -1., 0.25]).unwrap();
        let (l, r) = f.subdivide(0, 0.3);
        assert!((l.eval(&[0.5, 0.7]) - f.eval(&[0.15, 0.7])).abs() < 1e-13);
        assert!((r.eval(&[0.5, 0.7]) - f.eval(&[0.3 + 0.7 * 0.5, 0.7])).abs() < 1e-13);
        let (bl, br) = f.subdivide(1, 0.4);
        assert!((bl.eval(&[0.2, 0.5]) - f.eval(&[0.2, 0.2])).abs() < 1e-13);
        assert!((br.eval(&[0.2, 0.5]) - f.eval(&[0.2, 0.4 + 0.6 * 0.5])).abs() < 1e-13);
    }

    #[test]
    fn solves_circle_line_intersection() {
        // x^2 + y^2 = 0.5 and y = x on [0,1]^2: root at (0.5, 0.5).
        let u = [-0.25, -0.25, 0.75];
        let mut c1 = Vec::new();
        for a in u {
            for b in u {
                c1.push(a + b);
            }
        }
        let f1 = MultiBernstein::new(vec![2, 2], c1).unwrap();
        // f2 = y - x, degree (1,1): c[i][j] = j - i.
        let f2 = MultiBernstein::new(vec![1, 1], vec![0., 1., -1., 0.]).unwrap();
        let roots = solve_system(&[f1, f2], 1e-9, 100_000).unwrap();
        assert_eq!(roots.len(), 1, "{roots:?}");
        let cx = 0.5 * roots[0].lo[0] + 0.5 * roots[0].hi[0];
        let cy = 0.5 * roots[0].lo[1] + 0.5 * roots[0].hi[1];
        assert!((cx - 0.5).abs() < 1e-6 && (cy - 0.5).abs() < 1e-6, "{roots:?}");
    }

    #[test]
    fn no_roots_for_positive_system() {
        let f = MultiBernstein::new(vec![2, 2], vec![1.; 9]).unwrap();
        assert!(solve_system(&[f], 1e-9, 10_000).unwrap().is_empty());
    }

    #[test]
    fn two_circle_intersection() {
        // Unit-square scaled: circles centered (0.3, 0.5) and (0.7, 0.5),
        // radius 0.3: intersections at (0.5, 0.5 +- sqrt(0.05)).
        // f = (x-cx)^2 + (y-cy)^2 - r^2 as outer sum of univariates.
        let circ = |cx: f64, cy: f64, r: f64| -> MultiBernstein {
            // (x-c)^2 in Bernstein degree 2 from power [c^2, -2c, 1]:
            // b = [c^2, c^2 - c, c^2 - 2c + 1].
            let bx = [cx * cx, cx * cx - cx, cx * cx - 2.0 * cx + 1.0];
            let by = [cy * cy, cy * cy - cy, cy * cy - 2.0 * cy + 1.0];
            let mut c = Vec::new();
            for a in bx {
                for b in by {
                    c.push(a + b - r * r);
                }
            }
            // The r^2 constant is now subtracted once per term pair
            // from BOTH a and b sums; correct by adding it back once:
            // each entry should be a + b - r^2, which is what we built.
            MultiBernstein::new(vec![2, 2], c).unwrap()
        };
        let f1 = circ(0.3, 0.5, 0.3);
        let f2 = circ(0.7, 0.5, 0.3);
        let roots = solve_system(&[f1, f2], 1e-9, 200_000).unwrap();
        assert_eq!(roots.len(), 2, "{roots:?}");
        let dy = 0.05f64.sqrt();
        let mut ys: Vec<f64> = roots
            .iter()
            .map(|r| 0.5 * r.lo[1] + 0.5 * r.hi[1])
            .collect();
        ys.sort_by(f64::total_cmp);
        assert!((ys[0] - (0.5 - dy)).abs() < 1e-6, "{ys:?}");
        assert!((ys[1] - (0.5 + dy)).abs() < 1e-6, "{ys:?}");
        for r in &roots {
            let cx = 0.5 * r.lo[0] + 0.5 * r.hi[0];
            assert!((cx - 0.5).abs() < 1e-6);
        }
    }

    proptest! {
        // Univariate parity with the Bernstein root finder.
        #[test]
        fn univariate_matches_bernstein_roots(a in 0.1..0.9f64, b in 0.1..0.9f64) {
            prop_assume!((a - b).abs() > 1e-2);
            let p = crate::bernstein::Bernstein::from_power(&[a * b, -(a + b), 1.0]).unwrap();
            let want = p.roots(1e-12);
            // Bernstein degree-2 coefficients from power [c0, c1, c2]:
            // [c0, c0 + c1/2, c0 + c1 + c2].
            let c = vec![a * b, a * b - (a + b) / 2.0, a * b - (a + b) + 1.0];
            let f = MultiBernstein::new(vec![2], c).unwrap();
            let got = solve_system(&[f], 1e-9, 50_000).unwrap();
            prop_assert_eq!(got.len(), want.len(), "want {:?} got {:?}", &want, &got);
            let mut centers: Vec<f64> =
                got.iter().map(|r| 0.5 * r.lo[0] + 0.5 * r.hi[0]).collect();
            centers.sort_by(f64::total_cmp);
            for (g, w) in centers.iter().zip(want.iter()) {
                prop_assert!((g - w).abs() < 1e-6, "want {:?} got {:?}", &want, &centers);
            }
        }
    }
}
