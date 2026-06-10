# Tolerance Propagation Specification

Status: ADOPTED (specification + measurement program). Owner: kernel core.
Closes the corpus audit's loudest unresolved item (a-robust-numerics finding 6,
03-completeness-audit section 2.1, dossier 42 Theme 2): spec D2 promised
"per-entity local tolerances with propagation," the open literature does not
publish the propagation algorithms (they are proprietary), and the audit
verdict was that Keel must write its own specification BEFORE the model
calcifies. This document is that specification plus the empirical program
that validates it.

## 1. The model (the C3D formalization, dossier 42 Theme 2 Source 3)

Tolerance is bound to TOPOLOGY; geometry stays exact.

- A VERTEX with tolerance `tv` is the closed ball B(p, tv) about its point.
- An EDGE with tolerance `te` is the closed tube of radius `te` about its
  exact 3D curve.
- A FACE carries no own tolerance in this model: its boundary uncertainty is
  its loops' tubes; its interior is exact surface geometry.
- The session resolution `eps0` (SessionConfig) is the global floor: every
  per-entity tolerance satisfies `t >= eps0`.

Keel already stores `Edge.tolerance` (M7b, the Qi-Shapiro contract) and
`achieved_tolerance()`; this spec fixes the rules by which those values are
CREATED and GROW.

## 2. Containment invariants (checkable, validator-grade)

- I1 (vertex covers ends): for every edge e with end vertex v,
  `tv >= distance(v.point, e.curve_end)` and `tv >= te` is NOT required, but
  the ball must contain the tube's end disc: `tv >= te` whenever the curve
  end coincides with the vertex point; in general
  `tv >= |v.point - curve_end| + 0` and the pairing gap is absorbed per P2.
- I2 (tube covers truth): the true intersection locus of the two adjacent
  surfaces lies inside the edge tube. For certified SSI seams this is the
  certificate `tol_achieved` (times its safety factor); for exact analytic
  seams it is 0 (floored at eps0).
- I3 (floor): every tolerance `>= eps0`, and tolerances are MONOTONE under
  operations: no operation may shrink a tolerance without re-deriving the
  geometry that justifies it (re-intersection, P5).

## 3. Propagation rules (the original-engineering content)

- P1 (creation): a seam edge created from SSI carries
  `te = max(eps0, tol_achieved)`. Exact tier-1 curves carry `eps0`.
  Fitted/foreign edges carry their certificate.
- P2 (merge): when vertices at distance `d` merge (import glue, knit,
  stitch), the survivor carries `tv' = max(tv_a, tv_b) + d`. When edges glue
  into one radial cycle with maximum mutual deviation `g` (for coincident
  straight edges, the endpoint gaps), the survivor carries
  `te' = max(te_i) + g`. Rationale: ball/tube containment of both inputs'
  uncertainty regions; the additive `d`/`g` term is the conservative bound,
  no statistical discounting.
- P3 (dissolution): kef/kev/face-merge survivors inherit the MAX of the
  participants. Euler splits (split_edge/split_face) inherit the parent's
  tolerance unchanged on both children.
- P4 (transforms): isometries preserve tolerances; uniform scale by `s`
  multiplies them by `|s|`. Non-uniform maps are not closed over the model
  (declined today).
- P5 (re-intersection trigger): when a propagated tolerance would exceed
  `k * lfs(entity)` with `k = 0.01` (dossier 47 Q4; `lfs` = the
  inscribed-radius medial field, Addendum 161), the operation must
  RE-INTERSECT the adjacent geometry rather than widen further; if
  re-intersection cannot tighten below the threshold, DECLINE. This is the
  rule that prevents unbounded chained growth and the snap-vs-widen
  ambiguity the dossiers 39/47 flagged with inconsistent guidance.
- P6 (chaining bound, conjectured and MEASURED): under P1-P5 a chain of n
  booleans grows the max tolerance at most additively in the per-op gaps:
  `t_n <= max(t_0, max_op(tol_achieved)) + sum(gaps)`, with gaps bounded by
  the stitch vtol per op; in practice gaps are ~0 for clean models, so growth
  is FLAT until a genuinely tolerant contact occurs. This is the claim the
  measurement program tests, not assumes.

## 4. The measurement program (the audit's "turn missing literature into data")

The fuzz/billion-boolean infrastructure doubles as the experiment:

- M1: instrument the boolean pipeline (behind a cfg/feature or a recorder
  hook) to log per operation: max input `te`, max seam `tol_achieved`, max
  merge gap absorbed, max output `te`.
- M2: extend `fuzz_boolean` chains (the existing multi-op fuzz sectors) to
  depth 8-32 and record growth curves; the three-bucket oracle gains a
  TOLERANCE-GROWTH bucket: PASS requires the P6 bound to hold per chain.
- M3: any super-additive case is minimized into a repro and either fixed
  (a rule was violated) or the spec amended with the measured bound. The
  completion-gate soak (MEMORY: fuzz-soak-completion-gate) runs with M1
  enabled so the published kernel ships with measured, not asserted, growth
  characteristics.

## 5. Current conformance and gaps (honest inventory, 2026-06-10)

- CONFORMS: P1 (seam edges carry tol_achieved since M7b; the cylinder-
  cylinder rung and foreign fits carry certificates); P3 partially (splits
  inherit); P4 isometries.
- GAPS: P2 is IMPLICIT today (merge_and_glue absorbs gaps within vtol
  without recording them on the survivor); P5 exists only as the fixed
  boolean tolerance argument, not an lfs-relative trigger; M1-M3 not built.
  These are the named follow-ups; none changes the model.

## 6. Non-claims

No closed-form propagation theory is claimed; none exists in the open
literature (the audit's finding). This spec is a conservative containment
algebra plus an empirical validation program; the measured bounds are the
deliverable the literature lacks.
