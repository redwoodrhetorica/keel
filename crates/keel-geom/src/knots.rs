//! Clamped knot vectors: validation, span search, multiplicity.

use crate::{GeomError, MAX_DEGREE};

/// A validated, clamped (end multiplicity = degree + 1), nondecreasing
/// knot vector. Immutable after construction.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct KnotVector {
    degree: usize,
    knots: Vec<f64>,
}

impl KnotVector {
    /// Validate and construct. Requirements: 1 <= degree <= MAX_DEGREE,
    /// len >= 2 * (degree + 1), finite nondecreasing knots, clamped
    /// ends with multiplicity exactly degree + 1, interior multiplicity
    /// at most degree, and a nonempty domain.
    pub fn new(degree: usize, knots: Vec<f64>) -> Result<Self, GeomError> {
        Self::validate_core(degree, &knots)?;
        let p = degree;
        let m = knots.len() - 1;
        let (a, b) = (knots[0], knots[m]);
        // End multiplicity must be EXACTLY p + 1: a (p+2)-fold end value
        // puts a zero-width span inside the algorithms' index range (fuzz
        // finding 6: [a,a,a,b,b,b,b] at degree 2 made a derivative-curve
        // knot denominator exactly zero).
        if knots[p + 1] == a || knots[m - p - 1] == b {
            return Err(GeomError::MultiplicityExceeded);
        }
        // Interior values are capped at multiplicity p. Equal values are
        // contiguous (nondecreasing), so a violation is a run of p + 1
        // equal knots strictly between the clamped ends.
        if knots[p + 1..m - p].windows(p + 1).any(|w| w[0] == w[p]) {
            return Err(GeomError::MultiplicityExceeded);
        }
        Ok(Self { degree, knots })
    }

    /// Crate-internal: construct WITHOUT the multiplicity caps. For
    /// derivative (hodograph) knot vectors only, where an interior knot
    /// may legitimately carry multiplicity degree + 1: the derivative of
    /// a curve with a C0 corner is discontinuous there. Structural
    /// invariants (finite, nondecreasing, clamped, nonempty domain) are
    /// still enforced.
    pub(crate) fn new_hodograph(degree: usize, knots: Vec<f64>) -> Result<Self, GeomError> {
        Self::validate_core(degree, &knots)?;
        Ok(Self { degree, knots })
    }

    fn validate_core(degree: usize, knots: &[f64]) -> Result<(), GeomError> {
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
        Ok(())
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

    /// Knot span index k with `knots[k]` <= u < `knots[k+1]` (NURBS Book
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

    /// Fuzz finding 6: end multiplicity p + 2 ([a,a,a,b,b,b,b] at p = 2)
    /// passed validation and later produced a zero knot denominator.
    #[test]
    fn fuzz_regression_overfull_end_multiplicity_rejected() {
        assert_eq!(
            KnotVector::new(2, vec![0., 0., 0., 1., 1., 1., 1.]).unwrap_err(),
            GeomError::MultiplicityExceeded
        );
        assert_eq!(
            KnotVector::new(2, vec![0., 0., 0., 0., 1., 1., 1.]).unwrap_err(),
            GeomError::MultiplicityExceeded
        );
        // Interior multiplicity p + 1 is also out.
        assert_eq!(
            KnotVector::new(2, vec![0., 0., 0., 1., 1., 1., 2., 2., 2.]).unwrap_err(),
            GeomError::MultiplicityExceeded
        );
        // Interior multiplicity p is the legal maximum.
        assert!(KnotVector::new(2, vec![0., 0., 0., 1., 1., 2., 2., 2.]).is_ok());
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
