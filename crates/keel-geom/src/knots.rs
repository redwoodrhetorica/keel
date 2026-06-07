//! Clamped knot vectors: validation, span search, multiplicity.

use crate::{GeomError, MAX_DEGREE};

/// A validated, clamped (end multiplicity = degree + 1), nondecreasing
/// knot vector. Immutable after construction.
#[derive(Clone, Debug, PartialEq)]
pub struct KnotVector {
    degree: usize,
    knots: Vec<f64>,
}

impl KnotVector {
    /// Validate and construct. Requirements: 1 <= degree <= MAX_DEGREE,
    /// len >= 2 * (degree + 1), finite nondecreasing knots, clamped
    /// ends, and a nonempty domain.
    pub fn new(degree: usize, knots: Vec<f64>) -> Result<Self, GeomError> {
        if !(1..=MAX_DEGREE).contains(&degree) {
            return Err(GeomError::DegreeOutOfRange);
        }
        if knots.len() < 2 * (degree + 1) {
            return Err(GeomError::TooFewKnots);
        }
        if knots.iter().any(|k| !k.is_finite()) {
            return Err(GeomError::InvalidKnots);
        }
        if knots.windows(2).any(|w| w[1] < w[0]) {
            return Err(GeomError::InvalidKnots);
        }
        let p = degree;
        let m = knots.len() - 1;
        let (a, b) = (knots[0], knots[m]);
        if knots[p] != a || knots[m - p] != b || a >= b {
            return Err(GeomError::NotClamped);
        }
        Ok(Self { degree, knots })
    }

    #[inline]
    pub fn degree(&self) -> usize {
        self.degree
    }
    #[inline]
    pub fn knots(&self) -> &[f64] {
        &self.knots
    }
    /// Number of control points this knot vector pairs with.
    #[inline]
    pub fn control_count(&self) -> usize {
        self.knots.len() - self.degree - 1
    }
    /// Parameter domain [a, b].
    #[inline]
    pub fn domain(&self) -> (f64, f64) {
        (self.knots[0], self.knots[self.knots.len() - 1])
    }
    /// Clamp a parameter into the domain.
    #[inline]
    pub fn clamp(&self, u: f64) -> f64 {
        let (a, b) = self.domain();
        u.clamp(a, b)
    }

    /// Knot span index k with knots[k] <= u < knots[k+1] (NURBS Book
    /// A2.1). The end parameter returns the last nonempty span.
    pub fn find_span(&self, u: f64) -> usize {
        let p = self.degree;
        let n = self.control_count() - 1;
        let u = self.clamp(u);
        if u >= self.knots[n + 1] {
            return n;
        }
        let mut lo = p;
        let mut hi = n + 1;
        while hi - lo > 1 {
            let mid = (lo + hi) / 2;
            if u < self.knots[mid] {
                hi = mid
            } else {
                lo = mid
            }
        }
        lo
    }

    /// Multiplicity of value `u` among the knots (exact comparison).
    pub fn multiplicity(&self, u: f64) -> usize {
        self.knots.iter().filter(|&&k| k == u).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kv(p: usize, k: &[f64]) -> KnotVector {
        KnotVector::new(p, k.to_vec()).unwrap()
    }

    #[test]
    fn validation_rejects_bad_input() {
        assert_eq!(
            KnotVector::new(0, vec![0., 0., 1., 1.]).unwrap_err(),
            GeomError::DegreeOutOfRange
        );
        assert_eq!(
            KnotVector::new(1, vec![0., 0., 1.]).unwrap_err(),
            GeomError::TooFewKnots
        );
        assert_eq!(
            KnotVector::new(1, vec![0., 0., 1., 0.5]).unwrap_err(),
            GeomError::InvalidKnots
        );
        assert_eq!(
            KnotVector::new(2, vec![0., 0., 0.5, 1., 1., 1.]).unwrap_err(),
            GeomError::NotClamped
        );
        assert_eq!(
            KnotVector::new(1, vec![0., 0., 0., 0.]).unwrap_err(),
            GeomError::NotClamped
        );
        assert_eq!(
            KnotVector::new(1, vec![0., 0., f64::NAN, 1.]).unwrap_err(),
            GeomError::InvalidKnots
        );
    }

    #[test]
    fn span_search_basics() {
        // Degree 2, knots {0,0,0, 1, 2,2,2}: 4 control points.
        let k = kv(2, &[0., 0., 0., 1., 2., 2., 2.]);
        assert_eq!(k.control_count(), 4);
        assert_eq!(k.domain(), (0.0, 2.0));
        assert_eq!(k.find_span(0.0), 2);
        assert_eq!(k.find_span(0.5), 2);
        assert_eq!(k.find_span(1.0), 3);
        assert_eq!(k.find_span(1.5), 3);
        // End parameter maps into the last nonempty span.
        assert_eq!(k.find_span(2.0), 3);
        // Out-of-domain input is clamped.
        assert_eq!(k.find_span(99.0), 3);
        assert_eq!(k.find_span(-1.0), 2);
    }

    #[test]
    fn multiplicity_counts() {
        let k = kv(2, &[0., 0., 0., 1., 1., 2., 2., 2.]);
        assert_eq!(k.multiplicity(0.0), 3);
        assert_eq!(k.multiplicity(1.0), 2);
        assert_eq!(k.multiplicity(0.3), 0);
    }
}
