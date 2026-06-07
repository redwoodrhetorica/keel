//! Curves and surfaces for the Keel geometry kernel.
//!
//! Policy (spec D4): NURBS geometry lives in homogeneous 4D; all
//! algorithms are the non-rational ones run on `Vec4` control points,
//! with projection to 3D only at the API boundary. Analytic curve and
//! surface types are first-class and never silently converted to
//! NURBS for computation.

pub mod knots;

/// Maximum supported spline degree (spec D4: cap working degree;
/// Bernstein conditioning grows as p * 2^p, knot-independent).
pub const MAX_DEGREE: usize = 11;
/// MAX_DEGREE + 1: array capacity for one span's basis values.
pub const MAX_ORDER: usize = MAX_DEGREE + 1;

/// Errors from geometry construction and queries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GeomError {
    /// Degree outside 1..=MAX_DEGREE.
    DegreeOutOfRange,
    /// Knot vector too short for the degree and control count.
    TooFewKnots,
    /// Knots must be nondecreasing and finite.
    InvalidKnots,
    /// End knots must have multiplicity degree + 1 (clamped form).
    NotClamped,
    /// Control-point / weight counts inconsistent with knots.
    CountMismatch,
    /// Weights must be strictly positive and finite.
    InvalidWeight,
    /// Geometry is degenerate (zero direction, zero radius, ...).
    Degenerate,
    /// Parameter outside the curve domain.
    OutOfDomain,
    /// Knot multiplicity would exceed the degree.
    MultiplicityExceeded,
}
