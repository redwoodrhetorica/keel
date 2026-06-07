//! Numeric foundations for the Keel geometry kernel.
//!
//! Policy: tolerant modeling on f64 with exact predicates at decision
//! points. No combinatorial branch may read a raw f64 sign; use the
//! `predicates` module. Tolerances come from `tolerance`, never inline.
