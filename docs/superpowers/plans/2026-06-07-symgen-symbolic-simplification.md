# Symgen: Build-Time Symbolic Verification and Simplification Implementation Plan

> **STATUS: SHELVED (2026-06-07, user decision).** Plan only; do not execute.
> **Purpose (user clarification 2026-06-07): performance.** The trigger to
> pick this up is profiling/benches showing a hand-derived formula is a
> hot-path bottleneck and needs a faster algebraic form (Mode B is the
> point; Mode A ground-truth tables are the safety gate that lets a faster
> form ship, not a goal in themselves). The policy section is binding
> whenever this is picked up: sign-feeding expressions are never swapped,
> and no simplified form ships without ULP-parity fuzzing against the
> original plus a bench win.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A CAS-backed (SymPy) offline pipeline that (Mode A) verifies Keel's hand-derived hot-path formulas against symbolic ground truth, and (Mode B) generates CSE-simplified Rust for value-path expressions, gated by numerical-equivalence fuzzing and benchmarks before any simplified form ships.

**Architecture:** Python tooling under `tools/symgen/` is a DEV-TIME tool, never a build dependency: generated artifacts (golden tables, generated Rust) are committed to the repo, and a CI drift check re-runs generation and fails on diff (protobuf-codegen convention). Mode A emits 50-digit mpmath ground-truth sample tables consumed by Rust golden tests. Mode B emits CSE'd Rust modules that must pass a ULP-budget parity gate, a pathological-scale corpus (the M1/M2 fuzz-findings lessons), and a bench gate before replacing hand code.

**Tech Stack:** Python 3.12 + SymPy + mpmath (pinned), uv for env management; Rust side uses existing proptest/criterion/cargo-fuzz infra.

**Sequencing:** Infrastructure parallel to M3 (no dependency on keel-topo). First high-value consumer is M5, which needs third-order analytic derivative formulas for tangential SSI handling; deriving those by hand is exactly the error-prone work Mode B exists for.

---

## Policy (binding; goes in the doc and the code review checklist)

1. **Expression classes.**
   - **Sign-feeding** (predicates, discriminants that branch, anything whose sign or comparison reaches a combinatorial decision): NEVER swapped for a CAS-simplified form. These live under the determinism contract (spec D9 / kernel-14); only formally analyzed, contraction-off implementations are allowed. Mode A verification tables are still applied to them (verification cannot hurt).
   - **Value-path** (evaluators, derivatives, curvatures, Jacobian entries consumed by damped iterations): eligible for Mode B swap, gated as below.
2. **Symbolic equivalence is necessary, never sufficient.** Real-arithmetic identity does not imply floating-point fidelity. Every Mode B candidate must pass: (a) ULP-budget parity fuzzing against the reference implementation on domain-sampled inputs (budget: max 8 ULP relative deviation on well-conditioned samples, documented per expression if looser); (b) the pathological corpus (denormals, near-overflow magnitudes, near-degenerate configurations: every class the six fuzz findings taught us); (c) a criterion bench showing a measurable win, or an explicit non-performance justification (e.g. the formula did not previously exist). Failing any gate: the hand form stays and the generated form is kept only as a test oracle.
3. **Provenance.** Every generated artifact carries a header: generator script path, script content hash, SymPy version, generation date. The reference (hand) implementation is never deleted while a generated form ships; it moves to a `#[cfg(test)]` reference module as the permanent parity oracle.
4. **CAS choice.** SymPy is primary: open-source, scriptable, pinnable, CI-runnable. Wolfram is an OPTIONAL second opinion for exploratory work and cross-checks on the developer machine only (license cannot be a CI dependency of an OSS kernel). Where a formula is high-stakes, agreement of two independent CAS engines (SymPy + Wolfram) is the documented stronger standard, recorded in the artifact header.
5. **FP doctrine applies to generated code.** No `mul_add` emission, no reassociation of compensated sequences, plain `+ - * /` and `powi` only; `sin/cos/sqrt` routed through the same calls the hand code uses. The generator's Rust printer is configured accordingly and its output is clippy- and fmt-clean.

---

## File Structure

- Create: `tools/symgen/pyproject.toml`, `tools/symgen/README.md`
- Create: `tools/symgen/symgen/__init__.py`, `tools/symgen/symgen/emit.py` (shared emitters)
- Create: `tools/symgen/targets/surface_ders.py` (Mode A: analytic surface derivative ground truth)
- Create: `tools/symgen/targets/local_geometry.py` (Mode A: K/H/k1/k2 ground truth)
- Create: `tools/symgen/targets/torus_ders_gen.py` (Mode B pilot: CSE'd torus ders2)
- Create: `crates/keel-geom/tests/symgen_golden.rs` (consumes committed tables)
- Create: `crates/keel-geom/tests/data/symgen/*.json` (committed ground-truth tables)
- Create: `crates/keel-geom/src/generated/mod.rs`, `crates/keel-geom/src/generated/torus_ders.rs` (Mode B pilot output, feature-gated)
- Create: `fuzz/fuzz_targets/fuzz_symgen_parity.rs`
- Create: `tools/symgen/check_drift.ps1` (CI drift gate)
- Modify: `crates/keel-geom/src/lib.rs`, `fuzz/Cargo.toml`, `crates/keel-geom/benches/geom.rs`

---

### Task 1: symgen scaffold

**Files:** `tools/symgen/pyproject.toml`, `tools/symgen/README.md`, `tools/symgen/symgen/emit.py`

- [ ] **Step 1: pyproject with pinned deps**

```toml
[project]
name = "symgen"
version = "0.1.0"
requires-python = ">=3.12"
dependencies = [
    "sympy==1.13.3",
    "mpmath==1.3.0",
]
```

- [ ] **Step 2: shared emitters** (`tools/symgen/symgen/emit.py`)

```python
"""Shared emitters: deterministic JSON tables and Rust code with
provenance headers. Determinism matters: the CI drift check diffs
regenerated output byte-for-byte, so no dicts in arbitrary order, no
timestamps inside hashed content, fixed float formatting."""

import hashlib
import json
import pathlib
import sympy as sp


def provenance(script_path: str) -> dict:
    src = pathlib.Path(script_path).read_bytes()
    return {
        "generator": script_path.replace("\\", "/"),
        "script_sha256": hashlib.sha256(src).hexdigest(),
        "sympy_version": sp.__version__,
    }


def write_json_table(out_path: str, script_path: str, entries: list) -> None:
    """entries: list of dicts with stringified 50-digit decimal values.
    Values are strings so the table is exact and diff-stable."""
    doc = {"provenance": provenance(script_path), "entries": entries}
    text = json.dumps(doc, indent=1, sort_keys=True)
    pathlib.Path(out_path).write_text(text + "\n", newline="\n")


RUST_HEADER = """\
// GENERATED by {generator} (sha256 {script_sha256}, sympy {sympy_version}).
// DO NOT EDIT. Regenerate with: uv run python {generator}
// Policy: value-path expression; parity-gated against the reference
// implementation (see symgen plan, Policy section).
"""


def write_rust(out_path: str, script_path: str, body: str) -> None:
    head = RUST_HEADER.format(**provenance(script_path))
    pathlib.Path(out_path).write_text(head + body, newline="\n")


def rust_expr(e: sp.Expr) -> str:
    """SymPy -> Rust with the kernel FP doctrine: plain arithmetic,
    powi for small integer powers, no mul_add, f64 literals."""
    from sympy.printing.rust import RustCodePrinter

    class KeelPrinter(RustCodePrinter):
        def _print_Pow(self, expr):
            if expr.exp.is_Integer and 0 < expr.exp <= 4:
                base = self._print(expr.base)
                return f"({base}).powi({int(expr.exp)})"
            return super()._print_Pow(expr)

    return KeelPrinter().doprint(e)
```

- [ ] **Step 3: README** documenting the two modes, the policy section verbatim, and the regeneration command (`uv run python targets/<target>.py` from `tools/symgen/`).

- [ ] **Step 4: Commit.**

---

### Task 2: Mode A pilot, analytic surface derivative ground truth

The highest immediate value: M2b's `Surface3::ders2` closed forms were hand-derived. Symbolically differentiate the parameterizations and emit 50-digit sample tables; a Rust golden test then proves the hand formulas match symbolic ground truth to a few ULP.

**Files:** `tools/symgen/targets/surface_ders.py`, `crates/keel-geom/tests/data/symgen/surface_ders.json`, `crates/keel-geom/tests/symgen_golden.rs`

- [ ] **Step 1: the target script**

```python
"""Ground truth for Surface3 closed-form derivatives (Mode A).
Parameterizations exactly as documented in keel-geom surface.rs, in the
LOCAL frame (origin 0, axes e_x e_y e_z); the Rust test composes the
frame. Derivatives up to second order via sp.diff; values via mpmath
at 50 digits on a deterministic parameter grid."""

import itertools
import sys
import mpmath
import sympy as sp

sys.path.insert(0, "..")
from symgen.emit import write_json_table

mpmath.mp.dps = 50
u, v = sp.symbols("u v", real=True)

SURFACES = {
    # name: (S as 3-vector sympy Matrix, parameter dict variants)
    "cylinder": (
        lambda p: sp.Matrix([p["r"] * sp.cos(u), p["r"] * sp.sin(u), v]),
        [{"r": sp.Rational(2)}, {"r": sp.Rational(7, 10)}],
    ),
    "cone": (
        lambda p: sp.Matrix([
            (p["r0"] + v * sp.tan(p["alpha"])) * sp.cos(u),
            (p["r0"] + v * sp.tan(p["alpha"])) * sp.sin(u),
            v,
        ]),
        [{"r0": sp.Rational(1), "alpha": sp.pi / 4},
         {"r0": sp.Rational(3, 2), "alpha": sp.Rational(1, 2)}],
    ),
    "sphere": (
        lambda p: sp.Matrix([
            p["r"] * sp.cos(v) * sp.cos(u),
            p["r"] * sp.cos(v) * sp.sin(u),
            p["r"] * sp.sin(v),
        ]),
        [{"r": sp.Rational(3)}],
    ),
    "torus": (
        lambda p: sp.Matrix([
            (p["R"] + p["r"] * sp.cos(v)) * sp.cos(u),
            (p["R"] + p["r"] * sp.cos(v)) * sp.sin(u),
            p["r"] * sp.sin(v),
        ]),
        [{"R": sp.Rational(3), "r": sp.Rational(1)},
         {"R": sp.Rational(5, 2), "r": sp.Rational(4, 5)}],
    ),
}

# Deterministic, non-special parameter samples (avoid poles and axes;
# irrational-ish rationals so nothing simplifies accidentally).
US = [sp.Rational(3, 7), sp.Rational(11, 5), sp.Rational(31, 8)]
VS = [sp.Rational(-2, 5), sp.Rational(9, 13), sp.Rational(6, 5)]


def fifty(x) -> str:
    return mpmath.nstr(mpmath.mpf(sp.N(x, 60)), 50)


entries = []
for name, (build, variants) in sorted(SURFACES.items()):
    for pi, params in enumerate(variants):
        s = build(params)
        ders = {
            "s": s,
            "su": s.diff(u), "sv": s.diff(v),
            "suu": s.diff(u, 2), "suv": s.diff(u).diff(v), "svv": s.diff(v, 2),
        }
        for uu, vv in itertools.product(US, VS):
            entry = {
                "surface": name,
                "variant": pi,
                "params": {k: fifty(val) for k, val in sorted(params.items())},
                "u": fifty(uu),
                "v": fifty(vv),
            }
            for key, expr in ders.items():
                vals = expr.subs({u: uu, v: vv})
                entry[key] = [fifty(c) for c in vals]
            entries.append(entry)

write_json_table(
    "../../crates/keel-geom/tests/data/symgen/surface_ders.json",
    __file__,
    entries,
)
print(f"wrote {len(entries)} entries")
```

- [ ] **Step 2: run it** (`cd tools/symgen && uv run python targets/surface_ders.py`), commit the JSON.

- [ ] **Step 3: Rust golden test** (`crates/keel-geom/tests/symgen_golden.rs`)

```rust
//! Golden tests against CAS ground truth (symgen Mode A). The tables
//! are 50-digit mpmath values; the hand-derived closed forms must
//! match within a small ULP budget after f64 parse.

use keel_geom::surface::{Cone3, Cylinder3, Frame3, Sphere3, Surface3, Torus3};
use keel_math::vec::Vec3;
use serde_json::Value;

const TABLE: &str = include_str!("data/symgen/surface_ders.json");
/// Relative budget: closed forms vs correctly-rounded ground truth.
/// 4 ULP-ish; expressed as a relative epsilon for simplicity.
const REL: f64 = 1e-14;

fn close(a: f64, want: f64, scale: f64) -> bool {
    (a - want).abs() <= REL * (scale.abs().max(1.0))
}

#[test]
fn analytic_ders_match_cas_ground_truth() {
    let doc: Value = serde_json::from_str(TABLE).unwrap();
    let frame = Frame3::from_z(Vec3::ZERO, Vec3::new(0., 0., 1.)).unwrap();
    let mut checked = 0usize;
    for e in doc["entries"].as_array().unwrap() {
        let g = |k: &str| -> f64 { e[k].as_str().unwrap().parse().unwrap() };
        let gv = |k: &str| -> [f64; 3] {
            let a = e[k].as_array().unwrap();
            [
                a[0].as_str().unwrap().parse().unwrap(),
                a[1].as_str().unwrap().parse().unwrap(),
                a[2].as_str().unwrap().parse().unwrap(),
            ]
        };
        let p = &e["params"];
        let pf = |k: &str| -> f64 { p[k].as_str().unwrap().parse().unwrap() };
        let srf = match e["surface"].as_str().unwrap() {
            "cylinder" => Surface3::Cylinder(Cylinder3::new(frame.clone(), pf("r")).unwrap()),
            "cone" => Surface3::Cone(Cone3::new(frame.clone(), pf("r0"), pf("alpha")).unwrap()),
            "sphere" => Surface3::Sphere(Sphere3::new(frame.clone(), pf("r")).unwrap()),
            "torus" => Surface3::Torus(Torus3::new(frame.clone(), pf("R"), pf("r")).unwrap()),
            other => panic!("unknown surface {other}"),
        };
        let (u, v) = (g("u"), g("v"));
        // Surface3::ders2 is pub(crate); test through the public local
        // geometry plus point. If finer access is needed, promote a
        // doc-hidden accessor; do not weaken visibility silently.
        let lg = srf.local_geometry(u, v).unwrap();
        for (got, key) in [
            (lg.point, "s"), (lg.du, "su"), (lg.dv, "sv"),
            (lg.duu, "suu"), (lg.duv, "suv"), (lg.dvv, "svv"),
        ] {
            let want = gv(key);
            let scale = want.iter().fold(0.0f64, |m, w| m.max(w.abs()));
            assert!(
                close(got.x, want[0], scale)
                    && close(got.y, want[1], scale)
                    && close(got.z, want[2], scale),
                "{} {} at u={u} v={v}: got {:?} want {:?}",
                e["surface"], key, got, want
            );
        }
        checked += 1;
    }
    assert!(checked >= 50, "table suspiciously small: {checked}");
}
```

(Adds `serde_json` as a dev-dependency of keel-geom. The table is committed, so this test has zero Python dependency.)

- [ ] **Step 4: run, fix any mismatch.** A mismatch here is a REAL BUG in a hand formula, the entire point of Mode A. Investigate, never loosen REL past 1e-13 without a documented conditioning argument.

- [ ] **Step 5: Commit** generator + table + test.

---

### Task 3: Mode A, local geometry scalars (K, H, k1, k2)

**Files:** `tools/symgen/targets/local_geometry.py`, `crates/keel-geom/tests/data/symgen/local_geometry.json`, extend `symgen_golden.rs`

- [ ] **Step 1: script** mirrors Task 2 but computes, per surface/parameter sample, the first/second fundamental forms and K = (LN - M^2)/(EG - F^2), H, k1, k2 symbolically (sp.sqrt for the discriminant) at 50 digits. Emit `e,f,g,l,m,n,gaussian,mean,k1,k2` per entry. Same grid, same surfaces.

- [ ] **Step 2: Rust side** extends the golden test: compare `lg.{e,f,g,l,m,n,gaussian,mean}` at REL 1e-13 and `k1/k2` at 1e-7 near umbilics (documented: sqrt amplification of the discriminant, the M2b lesson) and 1e-12 elsewhere (an entry is "near-umbilic" when |k1 - k2| < 1e-6 * max(|k1|, |k2|, 1)).

- [ ] **Step 3: run, commit.**

---

### Task 4: Mode B pilot, CSE-generated torus ders2

Purpose: prove the full Mode B pipeline end to end on a low-risk target. Expectation, stated up front: the hand torus code already shares cos/sin subexpressions, so the bench gate will likely show no win and the generated form will NOT ship as the live path. That outcome still validates the pipeline (the gates correctly refusing a swap is the system working).

**Files:** `tools/symgen/targets/torus_ders_gen.py`, `crates/keel-geom/src/generated/{mod.rs,torus_ders.rs}`, parity tests

- [ ] **Step 1: generator**

```python
"""Mode B pilot: CSE'd torus ders2 in the local frame."""
import sys
import sympy as sp

sys.path.insert(0, "..")
from symgen.emit import rust_expr, write_rust

u, v, R, r = sp.symbols("u v big_r little_r", real=True)
s = sp.Matrix([
    (R + r * sp.cos(v)) * sp.cos(u),
    (R + r * sp.cos(v)) * sp.sin(u),
    r * sp.sin(v),
])
outs = []
names = []
for nm, e in [
    ("s", s), ("su", s.diff(u)), ("sv", s.diff(v)),
    ("suu", s.diff(u, 2)), ("suv", s.diff(u).diff(v)), ("svv", s.diff(v, 2)),
]:
    for i, comp in enumerate("xyz"):
        names.append(f"{nm}_{comp}")
        outs.append(sp.simplify(e[i]))

repl, reduced = sp.cse(outs, order="canonical")
lines = []
lines.append("#[allow(clippy::many_single_char_names)]")
lines.append("pub fn torus_ders2_local(u: f64, v: f64, big_r: f64, little_r: f64) -> [[f64; 3]; 6] {")
for sym, e in repl:
    lines.append(f"    let {sym} = {rust_expr(e)};")
for n, e in zip(names, reduced):
    lines.append(f"    let {n} = {rust_expr(e)};")
lines.append("    [")
for nm in ["s", "su", "sv", "suu", "suv", "svv"]:
    lines.append(f"        [{nm}_x, {nm}_y, {nm}_z],")
lines.append("    ]")
lines.append("}")
write_rust(
    "../../crates/keel-geom/src/generated/torus_ders.rs",
    __file__,
    "\n".join(lines) + "\n",
)
```

- [ ] **Step 2: wire the module** (`src/generated/mod.rs` with `pub mod torus_ders;`, declared in lib.rs; module docs state generated-code policy). Run `cargo fmt`/clippy on output; fix the PRINTER, not the output, if either complains.

- [ ] **Step 3: parity proptest** (in nurbs/surface test module or a tests/ file)

```rust
proptest! {
    // Generated vs reference over the live domain plus rough scales.
    #[test]
    fn generated_torus_ders_parity(
        u in -10.0..10.0f64, v in -10.0..10.0f64,
        big_r in 0.1..1e6f64, ratio in 0.01..0.99f64,
    ) {
        let little_r = big_r * ratio;
        let f = Frame3::from_z(Vec3::ZERO, Vec3::new(0., 0., 1.)).unwrap();
        let t = Surface3::Torus(Torus3::new(f, big_r, little_r).unwrap());
        let gen = keel_geom::generated::torus_ders::torus_ders2_local(u, v, big_r, little_r);
        let lg = t.local_geometry(u, v).unwrap();
        let pairs = [
            (lg.point, gen[0]), (lg.du, gen[1]), (lg.dv, gen[2]),
            (lg.duu, gen[3]), (lg.duv, gen[4]), (lg.dvv, gen[5]),
        ];
        for (hand, g) in pairs {
            let scale = 1.0 + hand.norm();
            // ULP-budget gate, expressed relatively (8 ULP ~ 2e-15).
            prop_assert!((hand.x - g[0]).abs() <= 2e-15 * scale);
            prop_assert!((hand.y - g[1]).abs() <= 2e-15 * scale);
            prop_assert!((hand.z - g[2]).abs() <= 2e-15 * scale);
        }
    }
}
```

Plus an explicit pathological-corpus test: the magnitude classes from fuzz findings (1e-308, 1e219, denormals) fed to both forms, asserting both produce identical finite/non-finite classification (neither form may be MORE fragile than the other).

- [ ] **Step 4: bench gate.** Criterion side-by-side `torus_ders_hand` vs `torus_ders_generated`. Record both in the LOG. Swap the live implementation ONLY if generated wins by >= 10% and all parity gates pass; otherwise the generated module stays `#[cfg(test)]`-consumed as an oracle and the pilot is recorded as pipeline-validated-no-swap.

- [ ] **Step 5: Commit.**

---

### Task 5: parity fuzz target

**Files:** `fuzz/fuzz_targets/fuzz_symgen_parity.rs`, `fuzz/Cargo.toml`

- [ ] **Step 1: target**

```rust
#![no_main]
use libfuzzer_sys::fuzz_target;

// Generated and hand forms must agree (relative ULP budget) or both
// be non-finite, for ARBITRARY f64 inputs including scale pathologies.
fuzz_target!(|data: (f64, f64, f64, f64)| {
    let (u, v, big_r, ratio) = data;
    if !(u.is_finite() && v.is_finite() && big_r.is_finite() && ratio.is_finite()) {
        return;
    }
    let big_r = big_r.abs();
    let little_r = big_r * ratio.abs().clamp(1e-9, 0.999);
    if !(big_r > little_r && little_r > 0.0) {
        return;
    }
    let f = match keel_geom::surface::Frame3::from_z(
        keel_math::vec::Vec3::ZERO,
        keel_math::vec::Vec3::new(0., 0., 1.),
    ) {
        Ok(f) => f,
        Err(_) => return,
    };
    let Ok(t) = keel_geom::surface::Torus3::new(f, big_r, little_r) else { return };
    let srf = keel_geom::surface::Surface3::Torus(t);
    let g = keel_geom::generated::torus_ders::torus_ders2_local(u, v, big_r, little_r);
    let Ok(lg) = srf.local_geometry(u, v) else { return };
    let hand = [lg.point, lg.du, lg.dv, lg.duu, lg.duv, lg.dvv];
    for (h, gg) in hand.iter().zip(&g) {
        for (a, b) in [h.x, h.y, h.z].iter().zip(gg) {
            if a.is_finite() != b.is_finite() {
                panic!("finite-class divergence");
            }
            if a.is_finite() {
                let scale = 1.0 + a.abs();
                assert!((a - b).abs() <= 1e-12 * scale, "parity {a} vs {b}");
            }
        }
    }
});
```

- [ ] **Step 2:** add the `[[bin]]`; 10-minute WSL soak per the established recipe is the ship gate for any Mode B artifact.

---

### Task 6: CI drift check and policy docs

**Files:** `tools/symgen/check_drift.ps1`, README updates

- [ ] **Step 1: drift script**

```powershell
# Regenerate all symgen artifacts and fail if the committed copies
# differ (the codegen-commit-diff convention). Run from tools/symgen.
$ErrorActionPreference = "Stop"
uv run python targets/surface_ders.py
uv run python targets/local_geometry.py
uv run python targets/torus_ders_gen.py
$st = git status --porcelain -- ../../crates/keel-geom/tests/data/symgen ../../crates/keel-geom/src/generated
if ($st) {
    Write-Error "symgen drift detected:`n$st"
    exit 1
}
"SYMGEN-DRIFT-CLEAN"
```

- [ ] **Step 2:** add the drift check and the parity fuzz soak to the milestone exit-gate checklist (alongside fmt/clippy/tests/fuzz). Document the Wolfram cross-check procedure: for high-stakes formulas, evaluate the same expressions in Wolfram on the same rational grid, export, and diff against the SymPy table; record agreement in the artifact header (`cross_checked: wolfram <version>`); never required in CI.

- [ ] **Step 3: Commit; LOG addendum.**

---

## Self-Review Notes

- **The dangerous failure mode is addressed structurally:** sign-feeding expressions are excluded from Mode B by policy (section 1), because real-equivalence does not survive rounding and the determinism contract forbids tolerance-band parity where signs branch. The plan never proposes simplifying predicates.
- **Mode A is the immediate payoff** (CAS verification of every hand-derived formula already shipped: surface ders, fundamental forms); Mode B is plumbing whose first real consumer is M5's third-order derivative formulas, where hand derivation is genuinely error-prone.
- **No Python in anyone's build:** artifacts are committed; CI verifies drift; consumers never need the toolchain.
- **Known gaps:** `Surface3::ders2` is pub(crate); the golden test goes through `local_geometry` instead, which is sufficient (it consumes all six derivatives). If a future target needs raw access, add a `#[doc(hidden)]` accessor deliberately.
- **Expectation management built in:** Task 4 predicts its own bench gate will refuse the swap; that is success, not failure.
