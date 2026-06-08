//! Non-manifold B-rep topology for the Keel geometry kernel.
//!
//! Governed by the M3 paper-design gate (docs/superpowers/specs/
//! 2026-06-07-m3-topology-gate-design.md): PES-class topology with
//! native space-partitioning regions, Euler-operator-only mutation,
//! total lineage reporting (spec D9), and deterministic identity.

pub mod arena;
pub mod body;
pub mod boolean;
pub mod construct;
pub mod entity;
pub mod euler;
pub mod imprint;
pub mod interrogate;
pub mod lineage;
pub mod massprops;
pub mod ops;
pub mod pmc;
pub mod query;
pub mod session;
pub mod simplify;
pub mod tessellate;
pub mod tweak;
pub mod validate;
pub mod winding;

pub use body::{Body, TopoError};
pub use entity::EntityId;
pub use lineage::{OpId, OpReport};
