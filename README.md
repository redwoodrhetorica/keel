# Keel

An open-source B-rep solid modeling kernel in Rust. NURBS and analytic
geometry, tolerant modeling on f64 with exact predicates at decision
points, and a roadmap to robust boolean operations.

Status: M1 (numeric foundations). Pre-alpha; APIs change without notice.

- Architecture spec: `docs/superpowers/specs/2026-06-07-keel-kernel-architecture-design.md`
- Research basis: `docs/research/00-synthesis.md`

## Layout

- `crates/keel-math`: vectors, matrices, transforms, intervals,
  tolerance policy, exact predicates, root finding, Bernstein machinery.
- `fuzz`: libFuzzer harnesses (run on Linux: `cargo +nightly fuzz run <target>`).

## Development

```
cargo test --workspace
cargo bench -p keel-math --bench math
cargo clippy --workspace --all-targets
```

License: MIT OR Apache-2.0.
