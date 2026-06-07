//! Session tolerance policy: the only module allowed to define epsilons.
//!
//! Model (spec D2): per-entity local tolerances are the primary model;
//! the session values here are the floor/default. Parasolid's published
//! figures are adopted: linear resolution 1e-8 in a normalized model
//! box, angular resolution 1e-11 radians. Per-entity tolerance values
//! appear on topology entities in M3 and are always >= the session
//! linear resolution.

/// Session-wide tolerance floor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tolerances {
    /// Linear resolution: distances below this are coincident.
    pub linear: f64,
    /// Angular resolution in radians.
    pub angular: f64,
}

impl Default for Tolerances {
    fn default() -> Self {
        Self {
            linear: 1.0e-8,
            angular: 1.0e-11,
        }
    }
}

impl Tolerances {
    /// Custom tolerances; None unless both are finite and positive.
    pub fn new(linear: f64, angular: f64) -> Option<Self> {
        let ok = |x: f64| x.is_finite() && x > 0.0;
        if ok(linear) && ok(angular) {
            Some(Self { linear, angular })
        } else {
            None
        }
    }
    /// |a - b| <= linear resolution.
    #[inline]
    pub fn eq_linear(self, a: f64, b: f64) -> bool {
        (a - b).abs() <= self.linear
    }
    /// |x| <= linear resolution.
    #[inline]
    pub fn is_zero_linear(self, x: f64) -> bool {
        x.abs() <= self.linear
    }
    /// |a - b| <= angular resolution (radians).
    #[inline]
    pub fn eq_angular(self, a: f64, b: f64) -> bool {
        (a - b).abs() <= self.angular
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_spec() {
        let t = Tolerances::default();
        assert_eq!(t.linear, 1.0e-8);
        assert_eq!(t.angular, 1.0e-11);
    }

    #[test]
    fn linear_equality_is_tolerant() {
        let t = Tolerances::default();
        assert!(t.eq_linear(1.0, 1.0 + 0.5e-8));
        assert!(!t.eq_linear(1.0, 1.0 + 2.0e-8));
        assert!(t.is_zero_linear(-0.9e-8));
    }

    #[test]
    fn angular_equality_is_tolerant() {
        let t = Tolerances::default();
        assert!(t.eq_angular(0.5, 0.5 + 0.5e-11));
        assert!(!t.eq_angular(0.5, 0.5 + 2.0e-11));
    }

    #[test]
    fn construction_rejects_nonpositive() {
        assert!(Tolerances::new(0.0, 1e-11).is_none());
        assert!(Tolerances::new(1e-8, -1.0).is_none());
        assert!(Tolerances::new(1e-6, 1e-9).is_some());
    }
}
