# Research Synthesis, Wave 3: Dossiers 12-19 (Adjacent Subsystems, IP, Determinism)

**Date:** 2026-06-07
**Inputs:** docs/research/kernel/12-19 (3D constraints/direct modeling, import healing, determinism/serialization, feature recognition, assembly tier, GD&T/PMI, patent landscape, parametric regeneration), plus kernel/11 (absorbed separately, see the wave-2 addendum). This wave is mostly about the BOUNDARY of the kernel: what must live inside, what the layers above need from it, and what the law constrains.

## Project-level decisions (applied to spec)

### P1. Patent posture (file 18, roadmap-binding)

Not legal advice; an FTO review is required before shipping affected features. Survey conclusions adopted as planning constraints:

- **The entire classical core is clear.** NURBS machinery, Euler operators/B-rep, classic booleans, Bezier clipping, classic blends including setback vertex and variable radius, adaptive tessellation, healing, subdivision surfaces: all expired or never-patented prior art. Implement the published versions and cite them (the discipline the survey calls the academic-publication safe harbor).
- **Three HIGH-risk zones, each with a published safe alternative:**
  1. *Auto-inferred constraints during direct edits* (Siemens Live Rules family: US 9,235,659 to ~2034, US 10,140,389, US 10,176,291). Safe path: direct editing with EXPLICITLY user-applied constraints; solve only user-specified relations. The file-12 solver architecture remains valid; the auto-infer-and-resolve behavior is the patented part.
  2. *Mixed mesh + B-rep faces in a single body operated on as one* (Siemens Convergent Modeling family, ~2035+). Safe path: mesh and B-rep as SEPARATE bodies, converting at well-defined boundaries via published mesh-arrangement booleans. The `FacetSurface` enum headroom stays (cheap, future-proof for patent expiry), but single-body convergent operations are off the shipping roadmap until the family lapses or counsel clears it.
  3. *U-splines* (Coreform, pending/active to ~2037). Safe path: THB-splines (published 2012, safe by construction) or classic T-splines, whose original patent (US 7,274,364) EXPIRED March 2024, a notable recent unlock.
- **License validated:** dual MIT OR Apache-2.0 (already our workspace license) inherits Apache's explicit contributor patent grant and retaliation clause; OCCT's license history shows how a bad license choice can strangle an ecosystem for a decade.
- **New standing rule: defensive publication.** Any novel Keel algorithm gets a dated public technical note so it cannot be enclosed by a later filing.

### P2. The differentiation thesis, now explicit (files 11 + 18 + 09)

Exact/certified topology decisions over tolerant metric geometry is the consensus recommendation of the literature AND unshipped by every commercial and open kernel. Combined with the affordability gradient (exact for conics, exact-when-cheap for quadrics, certified-numeric for splines), this is Keel's sharpest technical differentiation, and it is patent-clean (built entirely on published EGC/algebraic-kernel literature). Promoted into the spec mission statement.

## Binding deltas

### D14-contract: determinism and file format (file 14, operationalizes D9)

Adopted wholesale as the D9 implementation doctrine:

- **Determinism contract:** bitwise same-platform reproducibility mandatory; predicates exact with FMA contraction off; no fast-math anywhere including transitive C deps (and the thread-wide FTZ control-register hazard audited); transcendentals on determinism-critical paths via pure-Rust `libm` (or correctly-rounded RLIBM/CORE-MATH class routines) for cross-platform bit-identity; parallel results order-independent (stable IDs, write-to-index-slots, fixed-order or Demmel-Nguyen reproducible reductions); no HashMap iteration order, pointer values, or addresses ever influencing output (BTreeMap/IndexMap doctrine); FLiT-style reproducibility CI plus Verificarlo-style stability validation; tiered guarantee declared per algorithm (bitwise for predicates/IDs, reproducible reduction for saved scalars, tolerance-band only for display-only outputs).
- **File format doctrine:** schema-driven node list with integer-index references; persistent IDs in the file (naming survives reload); versioned header, append-only schema, format frozen across minor versions, down-save support, readers skip unknown entities/fields (XT + ACIS + Cap'n Proto consensus); two encodings (documented-grammar text for diffing, architecture-independent binary); exact f64 round-trip (raw bits/hex-float canonical, ryu shortest-round-trip for text); tolerances written into the file; importers treated as adversarial input (validate every count before allocating, fuzz continuously, partial recovery over panics); durable state is the definition, tessellation/regeneration are rebuildable cache; STEP AP242 is the must-implement neutral interchange.

### D-direct: the direct-modeling contract (file 12)

The solver is a peer beside the kernel (D-Cubed precedent), consuming (B-rep, drag, constraint scheme) and emitting new surface placements per face group. The kernel exposes exactly three things: `retweak(face -> new_surface)` (re-intersect with topology held), topology-event monitoring (Zou GTI tracking: detect face-graph changes along a drag, pause, apply local Euler ops/boolean, resume), and lineage hooks. Witness-method Jacobian rank is the only correct 3D constraint diagnosis (no Laman analog in 3D). Patent note from P1 applies to the inference half only.

### D-heal: healing pipeline (file 13)

Nine ordered phases: import + record source tolerance, diagnose (structured HealReport per PDQ/ISO 10303-59), stitch under stepped tolerance escalation with a hard cap, wire/edge fixes finest-to-coarsest (OCCT ordering), **simplify (recognize splines that are really analytics) BEFORE geombuild so re-intersection is exact**, rebuild geometry by re-intersection, heal-vs-tolerate policy (heal first, tolerate second, prefer re-tighten, record everything, bias exact for boolean-bound bodies), validate, lossy mesh-rescue fallback explicitly labeled. The surface-extension service is shared core (import healing, delete-face, tweak, gap fill) and must be co-designed with the SSI engine at M5.

### D-regen: what regeneration demands (file 19)

The feature tree, dependency DAG, rollback bar, scheduler, and cache policy all stay ABOVE the kernel (FreeCAD/OnShape/Grasshopper unanimity). The kernel owes: determinism, lineage, cheap snapshot/restore of body state, structured non-fatal operator failure (failed op leaves the body unchanged; host owns skip/suppress policy), **first-class datums/construction geometry and programmatic geometric selectors** (new requirement: by-position/adjacency/tag/condition queries with no GUI), concurrency-safe operators over disjoint bodies, and local ops that never assume a history exists. One kernel then serves history CAD, direct CAD, code-CAD, node-graph CAD, and collaborative CAD.

### D-asm: assembly tier boundary (file 16)

Kernel provides: instancing-friendly structural sharing (definition stored once, many occurrences reference it), a first-class cheap composable transform/location primitive, stable edit-surviving IDs, partition/transaction units, derived data on demand (tessellation at LOD, bounding volumes, exact pairwise clash/min-distance), and **local-coordinate geometry with double-precision placement (never bake world offsets into coordinates)**. Occurrence DAG, PLM structure, streaming/HLOD rendering, clash-at-scale orchestration, collaboration: all above.

### D-fr: feature-recognition substrate (file 15)

In-kernel: the attributed adjacency graph as a first-class queryable structure (faces + convex/concave/smooth/tangent edge classification: a hot path); **exact surface-type classification and canonical recovery ("this spline is actually a cylinder, axis/radius recovered"): the single highest-leverage kernel service for FR and shared with healing's simplify phase**; hint/verify primitives; UV-grid and coedge-walk export for ML (zero-conversion hooks). Above kernel: feature libraries, learned recognizers, application policies. Stances: hypotheses not verdicts (ranked interpretations), neuro-symbolic by construction (ML proposes, exact kernel verifies: consistent with D7), incremental recompute tied to persistent names.

### D-pmi: PMI readiness (file 17)

Pre-freeze obligations: PMI attaches to persistent entity references (PMI is the canonical consumer that proves the naming system); typed semantic PMI schema in the attribute system (characteristic enum, modifiers, ordered datum reference frame, ASME-vs-ISO flag), presentation separate; offset/virtual-condition solid generation (shared with shelling); substitute-geometry fitting subsystem with L2, Chebyshev/min-zone, and constrained (max-inscribed/min-circumscribed/tangent) objectives per NIST/ISO 10360 reference algorithms; signed distance with analytic derivatives for tolerance analysis; schema mirrors AP242/QIF for lossless export.

## Cross-wave convergence note

Persistent naming/lineage has now been independently demanded by SIX consumers: parametric regeneration (19), direct modeling (12), assemblies (16), PMI (17), feature recognition (15), and collaboration (07/14). D9 is the single most load-bearing non-geometric decision in the kernel.

## Reading-list updates

- Before M3 gate (additions): file 14 in full (determinism contract is implementation-ready); file 19 kernel-obligations list as the M3 API checklist; file 16 location/instancing contract.
- Before M5 (additions): file 13 extension-service requirements (co-design with SSI).
- Long tail: files 12, 15, 17 when their subsystems are scheduled; file 18 risk matrix re-checked (with counsel) before shipping any MEDIUM/HIGH feature.
