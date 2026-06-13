# README Fill-In Guide

Every `<!-- FILL: ... -->` placeholder in `README.md` is listed here with the evidence
needed to complete it and where to find that evidence.

Work through this list top-to-bottom before publishing or linking the page publicly.
All numeric claims must trace to a named LOG.md addendum or a test run artifact.
Round numbers DOWN per project doctrine (underclaim publicly).

---

## Badges (top of README)

| Placeholder location | What to put there | Evidence source |
|---|---|---|
| CI badge href | The real GitHub Actions workflow URL once the remote is live | `.github/workflows/ci.yml` + repo settings |
| License badge | Replace `TBD` with the chosen SPDX identifier | See License section below |
| Crates.io badge | Replace with real crate link once published | First `cargo publish` run |

---

## Honesty Contract section

### Randomized oracle testing

**Placeholder 1** (`<!-- FILL: total trial count and WRONG=0 record, source LOG Addendum XXX -->`):

- What to fill: The total number of randomized oracle trials run, and the WRONG=0
  confirmation with the specific LOG.md addendum number.
- Evidence: `LOG.md` Add.193 records the current gate state. Read it and cite the exact
  addendum: "Across N randomized trials, WRONG=0 (LOG Add.XXX)."
- Round N down to the nearest round figure (e.g. if 2.24 billion, write "over 2 billion").

**Placeholder 2** (`<!-- FILL: cite strict vs. tolerant lane counts from LOG Addendum XXX -->`):

- What to fill: The strict-lane and tolerant-lane trial counts and their WRONG=0 record.
- Evidence: Same addendum as above. The oracle runs two lanes (strict mass==mesh and
  tolerant). Cite both counts separately.

### Mass-mesh self-consistency gate

**Placeholder 3** (`<!-- FILL: cite gate threshold and the specific addendum -->`):

- What to fill: The numerical threshold used (e.g. relative tolerance on volume comparison)
  and the addendum where it was set.
- Evidence: Search `LOG.md` for "mass==mesh" or "gate threshold". Cite the addendum.

### Fuzz soak

**Placeholder 4** (`<!-- FILL: cite total fuzz execution count and soak duration -->`):

- What to fill: Total fuzz executions across all sectors and total wall-clock soak time.
- Evidence: `LOG.md` Add.193 and adjacent addenda record the fuzz-soak completion gate.
  Cite the specific addendum and the per-sector run counts.
- Example format: "over N billion executions across M hours of continuous soak (LOG Add.XXX)."

---

## Capabilities table

**Placeholder 5** (every `<!-- FILL: confirm from milestone record -->` cell in the Capabilities tables):

Go through the table row by row and set each status letter honestly:
- **S** = the feature is shipped, has tests, and has survived at least one fuzz or oracle run.
- **P** = code exists but certification is incomplete.
- **D** = declined by design (document why).
- Leave **?** only if you genuinely do not know the current state.

Evidence: `LOG.md` (the current anchor addendum), milestone branch history, test output.
Cross-reference the capabilities list in `docs/superpowers/specs/2026-06-07-keel-kernel-architecture-design.md`.

Rows most likely to need updating before publish:
- Cone and sphere booleans (see `LOG.md` sphere-split section and Add.XXX)
- Multi-body cellular boolean
- Chamfer
- STEP export and STEP import
- WASM build (the `keel-wasm` spike status)

---

## Verification section

### Oracle methodology

**Placeholder 6** (`<!-- FILL: e.g. 16/3 r^3 per closed form, cite LOG Addendum XXX -->`):

- What to fill: The exact closed-form volume for the Steinmetz (bicylinder) solid used
  as one of the analytic oracle references.
- Evidence: The Steinmetz volume for two cylinders of radius r intersecting orthogonally
  is 16r^3/3. Confirm the exact value used by the kernel in `LOG.md` (search "Steinmetz"
  or "bicylinder") and cite the addendum where this oracle was introduced.
- Write it as: "exactly 16r^3/3 (LOG Add.XXX)"

### Fuzz sectors

**Placeholder 7** (`<!-- FILL: cite current run counts per sector from LOG Addendum XXX -->`):

- What to fill: Per-sector run counts for `fuzz_boolean`, `fuzz_cyl_boolean`, `fuzz_imprint`,
  and any additional active sectors.
- Evidence: `LOG.md` fuzz-soak addenda. Add.193 (and nearby) records the completion gate
  with per-sector numbers. Cite each one separately.
- Round each count down to a round figure.

**Placeholder 8** (`<!-- FILL: cite any findings resolved and any known open items -->`):

- What to fill: A brief list of notable fuzz findings that were resolved (with addendum refs),
  and any currently known open items (with issue or addendum refs).
- Evidence: Search `LOG.md` for "FINDING" entries. Notable ones include:
  - fuzz_solve_cubic: extreme coefficient ratio overflow (see founding-era addenda)
  - fuzz_bernstein_roots: clean over 8.6M runs
  - fuzz_boolean: any findings from the soak campaign (check Add.193 and adjacent)

---

## Quickstart section

**Placeholder 9** (the minimal code example block):

- What to fill: A real, compilable, short Rust example demonstrating the most common
  first use case, probably: construct two bodies, run a boolean subtract, check the result
  is PASS or DECLINE, query volume.
- Evidence: Write this against the actual public API of `keel-topo` once the API stabilizes.
  Check `crates/keel-topo/src/lib.rs` for the current public surface.
- Rule: do not publish a code example that does not compile against the current crate.
  Keep it as a comment until it compiles.

---

## Roadmap table

**Placeholder 10** (every `<!-- FILL: e.g. complete, merged -->` cell in the Roadmap table):

- What to fill: Honest one-word status for each milestone.
- Evidence: `LOG.md` current anchor addendum. The anchor as of Add.193 is parity 137/144.
  Map that to milestone labels using the spec's milestone definitions.
- Suggested status words: "complete", "merged", "in progress", "planned", "not started".
- Do not mark a milestone complete unless it is merged to master and the gate test passed.

---

## License section

**Placeholder 11** (the entire License section):

- What to fill: Once the dual-licensing decision is made, replace the placeholder text with:
  - The chosen SPDX identifier (e.g. `MIT OR Apache-2.0` or a dual commercial arrangement).
  - A `LICENSE` file (or `LICENSE-MIT` + `LICENSE-APACHE`) at the repo root.
  - A contributing policy (CLA requirement or DCO, whichever is chosen).
  - Update `Cargo.toml` `workspace.package.license` to match.
- Current state: the workspace Cargo.toml still carries `MIT OR Apache-2.0` from early
  planning. Do not publish crates until the license decision is final.
- Note: T-splines are not used (patent cleared March 2024 for classical T-splines anyway,
  but dropped by design). The D10 patent posture in the spec is the governing document.

---

## Gallery captions

The curated gallery captions in the table are placeholder one-liners. Before publishing:

- Watch each `.webp` and write a caption that accurately describes what the demo shows.
- The `honest-decline` demo caption is especially important: it should explain what
  the decline means (the operation was refused, the input is unchanged) without making
  it sound like a failure.
- Do not use superlatives in any caption.

---

## Cross-checks before publish

1. Run `cargo test --workspace` and confirm all tests pass. Record the count in LOG.md.
2. Confirm the badge URLs resolve (CI badge, license badge, crates.io badge).
3. Confirm every `.webp` file referenced in the gallery table exists in `docs/media/`.
4. Confirm the `keel-wasm` status in the Capabilities table matches the current spike state.
5. Confirm the roadmap table matches `LOG.md` current anchor.
6. Confirm the license section is consistent with `Cargo.toml` and any `LICENSE` files present.
7. Read the full README aloud (or have a reviewer read it). Flag any superlative, any
   unsupported number, any em-dash, and any claim without a citation slot.

## Maintainer addendum (post-scaffold)

- RESOLVED (Add. 236 era): `honest-decline.webp` was regenerated from
  unequal-radius crossing cylinders (UnassemblableSeam), the sibling of
  steinmetz.webp (equal radii -> exact 16/3). It genuinely declines and
  self-guards if the kernel ever assembles it. Caption it as the
  decline-never-wrong contract on camera; no further action needed.
- The 22-item FILL inventory lives in this file above; every number
  must trace to a LOG.md addendum per the underclaim doctrine.
