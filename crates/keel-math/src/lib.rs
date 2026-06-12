//! Numeric foundations for the Keel geometry kernel.
//!
//! Policy: tolerant modeling on f64 with exact predicates at decision
//! points. No combinatorial branch may read a raw f64 sign; use the
//! `predicates` module. Tolerances come from `tolerance`, never inline.

pub mod algebraic;
pub mod bbox;
pub mod bernstein;
pub mod bigint;
pub mod interval;
pub mod mat;
pub mod multibernstein;
pub mod newton;
pub mod poly;
pub mod predicates;
pub mod tolerance;
pub mod transform;
pub mod vec;
