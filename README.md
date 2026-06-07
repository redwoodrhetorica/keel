# Keel

*It will keel.*

An open-source B-rep solid modeling kernel in Rust. NURBS and analytic
geometry, tolerant modeling on f64 with exact predicates at decision
points, non-manifold topology with first-class regions, and a roadmap
to robust boolean operations.

Status: M3 (topology layer). Pre-alpha; APIs change without notice.

- Architecture spec: `docs/superpowers/specs/2026-06-07-keel-kernel-architecture-design.md`
- Topology gate design: `docs/superpowers/specs/2026-06-07-m3-topology-gate-design.md`
- Research basis: `docs/research/`
- Project diary: `LOG.md`

## Layout

- `crates/keel-math`: vectors, matrices, transforms, intervals,
  tolerance policy, exact predicates, root finding, Bernstein machinery
  including the multivariate Projected Polyhedron solver.
- `crates/keel-geom`: NURBS curves and surfaces (homogeneous 4D policy),
  analytic curves and surfaces, exact surfaces of revolution, local
  differential geometry, global closest-point projection, interval
  enclosures.
- `crates/keel-topo`: PES-class non-manifold B-rep topology with native
  space-partitioning regions, Euler operators, lineage reporting,
  sessions, primitive constructors.
- `fuzz`: libFuzzer harnesses (run on Linux: `cargo +nightly fuzz run <target>`).

## Development

```
cargo test --workspace
cargo bench -p keel-geom --bench geom
cargo clippy --workspace --all-targets
```

License: MIT OR Apache-2.0.
