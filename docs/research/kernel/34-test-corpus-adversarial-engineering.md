# 34. Test Corpus and Adversarial Robustness Engineering

## Title and scope

This file is about one thing: how to **build, generate, and curate the corpus that hardens a B-rep kernel**. The project's standing thesis is that robustness in a geometry kernel is not primarily an algorithmic property but an **organizational asset**: the regression and stress corpus is the real robustness engine. C3D advertises a 500k+ model regression suite; Parasolid's robustness is widely understood to rest on a multi-decade accumulation of "every bug ever found, frozen as a test." For an open-source kernel, a public corpus is the single moat we can plausibly out-build a proprietary incumbent on, because the corpus compounds with every contributor's broken file.

This file is the **input side** of testing. It is deliberately scoped against three companions:

- `kernel/08-kernel-engineering` already gives the broad survey (proptest exists, cargo-fuzz exists, the ABC dataset exists, metamorphic and differential testing exist). This file does **not** repeat that survey. It goes deep on the engineering of generation and curation.
- `kernel/35` covers correctness **oracles** and verification: how you decide an answer is right. This file covers how you manufacture the **breaking inputs** that the oracles then judge. Where the two meet (metamorphic relations, known-answer constructions) this file looks from the input-generation side and cross-references.
- `kernel/14` covers determinism and serialization, which this file leans on for reproducible corpus runs.

Scope of this file: adversarial and degenerate geometry generation; grammar-based and structured generation of valid-but-pathological B-reps; property-based generator and shrinker design for geometric types; coverage-guided and continuous fuzzing of importers and the operation API; public datasets as corpus sources and their honest limits; failure triage and the named-regression discipline; metamorphic input-pair generation; the robustness pass-rate methodology; corpus scale and determinism; and generating geometry with analytically known answers.

The unifying principle, stated once up front so it can be referenced throughout, is the **coincidence-first mindset**. In random-float geometry, exact coincidence is a measure-zero event you will never hit by chance. In real CAD, exact coincidence is the **common case**: a hole drilled exactly on a face, two extrusions sharing a planar face, a fillet exactly tangent, a part mated flush against another. A boolean engine that only ever sees generic position in its tests is being trained on the wrong distribution. Therefore the corpus must be **biased toward coincidence and degeneracy**, not toward random position. Every generator in this file is designed to drive a configuration **into** its degenerate point, not away from it.

---

## 1. Adversarial and degenerate geometry generation

The failure modes of NURBS booleans and tolerant edges are specific and enumerable, and that enumeration is itself the generation specification. The taxonomy below doubles as a checklist of generators to write.

**The failure-mode taxonomy (each is a generator target).**

- *Near-tangency*: two surfaces whose intersection curve has a point where they are tangent. The surface-surface intersection (SSI) marcher loses its transversality assumption; the intersection curve can pinch, split, or spuriously self-touch. Generator: take two transverse surfaces, parameterize a separation/angle, and sweep it to the tangent configuration.
- *Near-coincidence*: two surfaces that are almost the same surface over a region (two cylinders of nearly equal radius and axis; two planes at micro-angle). The SSI degenerates from a curve to a 2D region; classification of "in/out/on" becomes ill-posed. Generator: perturb one surface from an exact copy of the other by epsilon.
- *Slivers and knife-edges*: faces or solids with an aspect ratio approaching infinity, where one dimension is below tolerance while another is large. These break edge-merge decisions and tolerance-vs-feature-size assumptions.
- *Grazing contacts*: a solid that touches another along a curve or at a point but does not penetrate (a sphere resting on a plane; a cylinder tangent to a box face). The boolean must decide whether the contact produces a shared edge, a non-manifold vertex, or nothing.
- *Nearly-singular intersections*: intersection curves passing through a surface singularity (cone apex, sphere pole, the seam of a closed surface, a degenerate NURBS edge where control points collapse).
- *Tiny features below tolerance*: a chamfer, hole, or boss smaller than the modeling tolerance. The kernel must decide consistently whether the feature exists.
- *Near-degenerate parameterizations*: NURBS with near-zero-length knot spans, near-coincident control points, near-zero weights, or wildly non-uniform parameterization that makes the Jacobian nearly singular.
- *High genus*: many through-holes, so the boolean produces many disjoint intersection loops that must be correctly threaded into faces.

**Parametric degenerate families.** The single most productive generator design is the **swept family**: a function `config(t)` that returns a geometric configuration, where `t = t*` is exactly degenerate and `t` near `t*` is near-degenerate. Examples: two boxes whose overlap depth goes to zero; a fillet radius sweeping to exactly the half-thickness of a plate (where the fillet consumes the whole face); two cylinders whose axes converge to coaxial. You then sample `t` on a logarithmic grid straddling `t*` (e.g. `t* ± {1, 1e-3, 1e-6, 1e-9, 1e-12, 0}`). This gives a continuum from "clearly generic" to "exactly degenerate," and a robust kernel should produce a **topologically consistent answer that varies continuously** across the family, with the exactly-degenerate case being the natural limit. Discontinuities in the topology as `t -> t*` are the bug signature.

**The coincidence-first generation principle, made concrete.** Rather than generate two independent random solids and union them (generic position, low value), generate one solid, then construct the second **by a relation guaranteed to produce coincidence**: copy a face and use it as a cutting plane; translate the solid by exactly an edge length so faces align; reflect across a face. Snap all generated coordinates to a coarse grid so that independently generated features land on coincident planes. The corpus distribution should put the bulk of its mass on the configurations a random generator would assign measure zero.

### Source 1.1: Kettner, Mehlhorn, Pion, Schirra, Yap, "Classroom Examples of Robustness Problems in Geometric Computations"

**Citation.** L. Kettner, K. Mehlhorn, S. Pion, S. Schirra, C.-K. Yap. *Classroom Examples of Robustness Problems in Geometric Computations.* Computational Geometry: Theory and Applications, 40(1):61-78, 2008. (Earlier ESA 2004.) https://www.sciencedirect.com/science/article/pii/S0925772107000697 and https://inria.hal.science/inria-00344310/

**Technique.** The paper takes two textbook algorithms (2D convex hull, 3D Delaunay) implemented with floating-point arithmetic and constructs concrete inputs that make them fail in many distinct ways: produce non-convex hulls, crash, loop forever, or emit topologically impossible output. Crucially, it shows how to **construct such inputs systematically** by reasoning about the floating-point geometry of the orientation predicate: where the sign of the predicate flips incorrectly relative to the exact sign, and arranging points to sit precisely in those error bands. The degenerate configurations are nearly-collinear or nearly-cocircular point sets, the 2D analog of near-tangency.

**Limitations.** The case studies are low-dimensional (points, hulls, triangulations), not NURBS booleans. The constructive method assumes you can analyze the predicate's floating-point behavior, which is harder for a marching SSI than for a 3x3 orientation determinant.

**Kernel relevance.** This is the foundational argument that **degenerate inputs are constructible, not just discoverable by luck**, and that the construction targets the predicate's error band. For Keel this justifies a generator that takes a geometric predicate (point-on-surface, orientation, in/out classification) and synthesizes inputs that land in its floating-point ambiguity zone. It also motivates exact or filtered-exact predicates (the project's predicate work) by showing exactly how naive float predicates break. Every "classroom example" should become a named regression case.

---

## 2. Grammar-based and structured generation

Random bytes never parse into a valid B-rep, so byte-mutation fuzzers waste essentially all their budget in the parser's reject path. The fix is to generate inputs from a **grammar** or a **constructive sequence** that is valid by construction, then push that valid object toward degeneracy. The CAD-ML literature has, as a side effect of building training data, produced exactly the construction-sequence generators a kernel test corpus wants.

**Two generator philosophies.**

1. *Construct valid, then perturb to degeneracy.* Build a solid by a random but valid **feature history** (sketch, extrude, fillet, hole, pattern). The result is guaranteed manifold and valid because each Euler-operator-respecting step preserves validity. Then apply a perturbation that drives one feature toward a degenerate limit (radius to zero, two holes toward coincidence). This is the highest-yield structured generator: it produces solids that *look like real parts* and fail in *real-CAD ways*.
2. *Grammar over the operation API.* Treat the kernel's public operations as a grammar's terminals and generate random well-typed **operation sequences** (create primitive, boolean, fillet, offset, imprint, sew). This is structured fuzzing of the API rather than the importer, and it finds state-machine bugs no single-shot input can.

### Source 2.1: Wu, Xiao, Zheng, "DeepCAD: A Deep Generative Network for CAD Models"

**Citation.** R. Wu, C. Xiao, C. Zheng. *DeepCAD: A Deep Generative Network for Computer-Aided Design Models.* ICCV 2021. https://openaccess.thecvf.com/content/ICCV2021/papers/Wu_DeepCAD_A_Deep_Generative_Network_for_Computer-Aided_Design_Models_ICCV_2021_paper.pdf

**Technique.** DeepCAD models a CAD solid as a **sequence of construction commands** (sketch primitives: line, arc, circle; then extrude with parameters), and trains a transformer to generate such sequences. The dataset (DeepCAD-180k, derived from ABC via the Onshape/FeatureScript pipeline) is a corpus of sketch-and-extrude command sequences with known parameters. The key reusable artifact for testing is the **command schema and the deterministic command-to-B-rep interpreter**: a compact grammar (a handful of sketch primitives plus extrude/boolean) that generates valid solids.

**Limitations.** Restricted to sketch-and-extrude with boolean union/cut/intersect; no fillets, lofts, sweeps, or freeform NURGS patches in the command vocabulary. Generated sequences can be self-intersecting or produce invalid sketches that the interpreter must reject. The ML generator's distribution is biased toward "typical mechanical parts," not toward degeneracy, so it must be paired with the perturbation step from philosophy (1).

**Kernel relevance.** The DeepCAD command grammar is a **ready-made constructive generator** for Keel's test harness. Implement the interpreter against Keel's own modeling API and you get an unbounded stream of valid solids whose construction history is known (useful for persistent-naming and regeneration tests too, cf. `kernel/07`, `kernel/19`). Bias the parameter sampler toward coincident planes and zero-extent extrudes to make it adversarial. The construction sequence is also the natural **shrink target**: shrink the command list, not the resulting B-rep.

### Source 2.2: Seff, Ovadia, Zhou, Adams, "SketchGraphs: A Large-Scale Dataset for Modeling Relational Geometry in CAD"

**Citation.** A. Seff, Y. Ovadia, W. Zhou, R. P. Adams. *SketchGraphs: A Large-Scale Dataset for Modeling Relational Geometry in Computer-Aided Design.* ICML 2020 Workshop on Object-Oriented Learning. https://github.com/PrincetonLIPS/SketchGraphs and the EmergentMind overview.

**Technique.** 15 million real 2D CAD sketches scraped from Onshape, each represented as a **geometric constraint graph**: primitives (points, lines, arcs, circles) as nodes and constraints (coincident, perpendicular, parallel, tangent, equal, distance) as edges. The dataset ships a parser, a renderer, and a sequence representation of the sketch-construction operations.

**Limitations.** 2D only: it is a sketch dataset, not a solid dataset. Constraint graphs may be under- or over-constrained. To get a solid you must extrude/revolve, which SketchGraphs does not provide.

**Kernel relevance.** Keel's 2D constraint solver (`kernel/04`) and profile machinery get a 15M-case real-world stress corpus directly, including the **coincidence-heavy** constraint distributions (`coincident` and `tangent` are among the most common constraints, which is the coincidence-first principle showing up in real data). As a 3D generator input, SketchGraphs sketches feed an extrude/revolve stage to manufacture solids whose base sketch came from a human, giving more realistic profiles than random polygon generators. The `tangent` constraint density is exactly the near-tangency adversarial source you want.

### Source 2.3: Para et al., "SkexGen" and the autoregressive construction-sequence line

**Citation.** W. Para et al. *SkexGen: Autoregressive Generation of CAD Construction Sequences with Disentangled Codebooks.* ICML 2022. https://arxiv.org/pdf/2207.04632 . Related: *Brep2Seq* (J. Comp. Design Eng. 2024, https://academic.oup.com/jcde/article/11/1/110/7582276).

**Technique.** SkexGen factors CAD generation into disentangled codebooks (topology, geometry, extrusion) and generates construction sequences autoregressively. The reusable idea for testing is **disentangled control**: you can hold the topology fixed and sweep the geometry, or hold geometry and vary the extrusion, which is precisely the parametric-family generator from Section 1 but driven by a learned model. Brep2Seq goes the other direction (B-rep to sequence), which is useful for turning a found-in-the-wild solid into a replayable, shrinkable construction sequence.

**Limitations.** Same sketch-and-extrude ceiling as DeepCAD; learned models add a dependency and a license/reproducibility burden you may not want in a kernel CI. Generated sequences need validation before use.

**Kernel relevance.** Disentangled sweeping is the design pattern to **steal without the ML**: structure Keel's generator so topology, geometry, and operation parameters are independently sampleable, enabling targeted degenerate-family sweeps. Brep2Seq's B-rep-to-sequence direction is the blueprint for the triage tool that converts a raw failing STEP file into a minimal reproducing construction script.

---

## 3. Property-based generator and shrinker design for geometry

"Use proptest" is not a design. The hard part is writing **generators that produce valid-but-interesting geometric values** and **shrinkers that shrink toward the degenerate witness**, which is the opposite of the default numeric shrink-toward-zero behavior.

**Generators for geometric types.**

- *Knot vectors*: a valid clamped knot vector is non-decreasing with correct end multiplicity. The generator should sample the **number of internal knots, the multiplicities (to hit C0/C1 reductions), and the span ratios (to hit near-zero spans)**. Sampling multiplicities up to degree creates the near-singular parameterizations of Section 1.
- *Control nets*: sample a grid, then bias toward **near-coincident adjacent control points** (collapses span length) and **near-zero or negative-ish weights** (the rational-blowup cases). Generate symmetric/planar nets to hit the "should-be-canonical" recovery paths (`kernel/24`).
- *Solids*: do **not** generate B-reps as random vertex/edge/face soup; that path produces invalid topology almost surely. Generate via the constructive grammar (Section 2) so validity is free, and let the generator parameters (positions, radii) be the proptest-sampled values.

**Shrinker design: shrink toward the witness, not away.** The default integer/float shrinker moves values toward zero and simplicity. For geometry the *interesting* failing input is usually the **most degenerate** one, so the shrinker must preserve or intensify the degeneracy that triggered the failure. Concretely:

- Shrink the **construction sequence** (drop operations, simplify primitives) rather than the resulting coordinates, so each shrink step yields a still-valid solid. This is the integrated-shrinking insight applied to geometry: shrink the recipe, regenerate the object.
- When shrinking coordinates, shrink **toward the coincidence**, e.g. snap a near-coincident pair to exactly coincident and check whether the failure persists, which both minimizes and sharpens the reproducer.
- Preserve invariants during shrinking (a shrunk knot vector must stay valid), which is exactly why integrated shrinking via the generator is preferable to type-based shrinking that can produce invalid intermediate values.

### Source 3.1: Claessen and Hughes, "QuickCheck: A Lightweight Tool for Random Testing of Haskell Programs"

**Citation.** K. Claessen, J. Hughes. *QuickCheck: A Lightweight Tool for Random Testing of Haskell Programs.* ICFP 2000. (Foundational PBT paper.)

**Technique.** Properties as universally quantified predicates; `Arbitrary` typeclass for random generation; user-defined generators via combinators; type-directed **shrinking** to minimize counterexamples. Establishes the separation of generator, property, and shrinker, and the idea that the framework's value is in turning a random failure into a *small* failure automatically.

**Limitations.** Classic QuickCheck shrinking is **type-based** and separate from generation, so a custom generator with invariants (a valid knot vector) needs a hand-written, invariant-preserving shrinker, which is error-prone. Pure random generation under-samples rare configurations (it will essentially never produce exact coincidence).

**Kernel relevance.** The generator/property/shrinker decomposition is the skeleton of Keel's PBT layer (proptest implements this model for Rust). The known weakness (random under-samples degeneracy) is exactly why Keel must hand-design coincidence-biased generators rather than rely on uniform sampling, and why the shrinker must be custom and invariant-preserving.

### Source 3.2: MacIver et al., Hypothesis and integrated/internal shrinking

**Citation.** D. R. MacIver et al. *Hypothesis* (Python). See "Property-based Testing: Shrinking Choices, Shrinking Values" (https://getcode.substack.com/p/property-based-testing-5-shrinking) and the falsify writeup (https://well-typed.com/blog/2023/04/falsify/) for the integrated-shrinking model; comparative analysis at https://seelengrab.github.io/articles/.

**Technique.** Hypothesis shrinks the **stream of random choices** (the PRNG bytes the generator consumed), not the generated value. Because every shrunk byte-stream is re-run through the *same generator*, the shrunk value is **valid by construction**: you can never shrink a valid knot vector into an invalid one, because the generator that enforces validity is in the loop on every shrink. Hypothesis also provides **targeted PBT** (steer generation to maximize a user-supplied score), **swarm testing** (randomly disable some features per run to escape generation bias), and **error deduplication**.

**Limitations.** Integrated shrinking can shrink along the wrong axis if the generator's choice structure does not align with "interesting"; you sometimes get a syntactically minimal but semantically odd reproducer. Targeted PBT needs a meaningful score function.

**Kernel relevance.** This is the **most important PBT design lesson for Keel**: build generators so that shrinking the input choices automatically yields valid geometry, removing the need for fragile hand-written geometric shrinkers. Rust's proptest supports this style (strategies compose and shrink through the generator). **Targeted PBT is the bridge to Section 1's degenerate families**: define the score as "proximity to the degenerate configuration" (e.g. negative of the minimum face angle, or the inverse of the smallest knot span) and Hypothesis-style targeting will *drive generation into the degeneracy* automatically rather than waiting for luck. Swarm testing maps to "randomly enable/disable kernel operations per sequence" to find operation-interaction bugs.

### Source 3.3: Hughes, "Experiences with QuickCheck: Testing the Hard Stuff and Staying Sane" (stateful/model-based PBT)

**Citation.** J. Hughes. *Experiences with QuickCheck: Testing the Hard Stuff and Staying Sane.* 2016. https://www.cs.tufts.edu/~nr/cs257/archive/john-hughes/quviq-testing.pdf . Implementations: quickcheck-state-machine, quickcheck-dynamic (https://github.com/input-output-hk/quickcheck-dynamic); Rust analog: proptest-state-machine.

**Technique.** **Stateful (model-based) property testing.** Instead of one input to one function, generate a **sequence of commands** against a stateful system, run them against both the real system and an abstract **model**, and assert the observable results agree after each step. Generators produce command sequences (with preconditions so only valid commands are emitted in a given state); shrinking removes commands from the sequence while preserving validity. Hughes' war stories show this finding deep bugs that single-call testing cannot, because the bug only manifests after a specific operation interleaving.

**Limitations.** Requires a faithful abstract model, which for a full kernel is itself a research problem; you typically model a *property* of the state (validity, Euler-Poincare invariant, volume monotonicity under specific ops) rather than the full geometry. Shrinking command sequences is more expensive than shrinking values.

**Kernel relevance.** This is the right frame for testing **operation sequences** (boolean, then fillet, then offset, then imprint). The "model" need not reproduce geometry; it can assert cheap invariants after every step: the solid stays valid (manifold, closed, consistent orientation), Euler-Poincare holds, volume stays non-negative, and topology counts are sane. A randomly generated, shrinkable command sequence that drives Keel into an invalid state is the most valuable kind of bug report, and the shrunk sequence is a ready-made regression test. This directly operationalizes "generate solids by random feature histories then perturb."

---

## 4. Coverage-guided and continuous fuzzing

Fuzzing a kernel splits cleanly into two surfaces: the **importers** (untrusted parsers of STEP/IGES/STL, classic memory-safety fuzzing) and the **operation API** (structured fuzzing where the "input" is a typed geometric object plus an operation). The two need different harnesses.

### Source 4.1: Zalewski, AFL, and the coverage-guided greybox model

**Citation.** M. Zalewski. *American Fuzzy Lop (AFL).* See also AFL++ (Fioraldi et al., WOOT 2020, https://www.usenix.org/system/files/woot20-paper-fioraldi.pdf).

**Technique.** Greybox fuzzing: instrument the target for edge coverage, keep a queue of inputs, mutate them (bit flips, splices, dictionary tokens), and **retain any mutant that hits a new edge**, evolving a corpus that maximizes coverage. AFL++ adds custom mutators, persistent-mode speed, and the `afl-cmin`/`afl-tmin` tooling for corpus and testcase minimization.

**Limitations.** Pure byte-mutation is near-useless against a structured parser: almost every mutant fails an early length/magic-number check and never reaches the geometry. Edge coverage is a poor proxy for **configuration-space** coverage in a kernel (you can have 100% code coverage and have never tested two coincident faces).

**Kernel relevance.** AFL/AFL++ is the right tool **only with a structured input model** (a custom mutator or a grammar front-end). The genuinely valuable AFL artifacts for Keel are `afl-cmin` (corpus minimization, Section 9) and `afl-tmin` (testcase minimization for triage, Section 6). Use AFL on the importer surface seeded with real STEP files; do **not** expect raw AFL to meaningfully exercise the operation API.

### Source 4.2: libFuzzer and structure-aware / structured fuzzing

**Citation.** *libFuzzer.* LLVM docs: https://llvm.org/docs/LibFuzzer.html . Structure-aware fuzzing guide: https://github.com/google/fuzzing/blob/master/docs/structure-aware-fuzzing.md . Rust: `cargo-fuzz` + the `arbitrary` crate.

**Technique.** libFuzzer is an in-process coverage-guided fuzzer driven by a `LLVMFuzzerTestOneInput(bytes)` entry point. **Structure-aware fuzzing** wraps the byte buffer in a decoder (`libprotobuf-mutator`, or Rust's `arbitrary::Arbitrary` derive) so the fuzzer mutates a **typed structure** rather than raw bytes, and the mutations stay within the type's grammar. In Rust, you `#[derive(Arbitrary)]` on a struct and the fuzzer fills it from the coverage-guided byte stream; libFuzzer's `-merge=1` performs coverage-preserving corpus minimization (set-cover based in recent LLVM).

**Limitations.** Structure-aware fuzzing only constrains *syntax*; it does not guarantee *semantic* validity (an `Arbitrary` knot vector may be non-monotone) unless your decoder enforces invariants. In-process fuzzing requires the target not to corrupt global state between runs (a determinism requirement, cf. `kernel/14`).

**Kernel relevance.** This is the **core fuzzing recipe for Keel's operation API**: define `#[derive(Arbitrary)]` decoders for geometric inputs (a `Solid` built via the construction grammar from a byte stream, an operation tag, parameters) so the fuzzer explores **valid geometric configurations under coverage guidance**, biased toward coincidence by the decoder. The decoder is where the coincidence-first principle lives: have `Arbitrary` snap coordinates to a coarse grid and reuse already-emitted planes, so the fuzzer naturally produces coincident geometry. `cargo-fuzz` over importers (seeded with ABC/Thingi10K) is the memory-safety net; `cargo-fuzz` over the `Arbitrary`-decoded operation API is the logic-bug net. Use `-merge=1` for corpus minimization.

### Source 4.3: OSS-Fuzz continuous infrastructure

**Citation.** *OSS-Fuzz* (Google). https://google.github.io/oss-fuzz/ . By Feb 2023 OSS-Fuzz reported 8,900+ vulnerabilities and 28,000+ bugs across 850+ projects.

**Technique.** Continuous fuzzing as infrastructure: projects register fuzz targets and a build config; OSS-Fuzz runs them 24/7 on Google hardware, maintains and minimizes corpora, **deduplicates crashes by stack hash**, files and tracks bugs, and **verifies fixes** automatically. Provides ClusterFuzz's triage, bisection, and corpus-management services for free to qualifying OSS projects.

**Limitations.** Requires the project to be open and to maintain libFuzzer/AFL targets; the geometry-specific semantic bugs (wrong-but-not-crashing boolean) are invisible to a crash-only fuzzer unless you add assertion-based oracles (validity asserts) to turn semantic errors into crashes.

**Kernel relevance.** Keel is open-source and should **apply to OSS-Fuzz** for the importers and the `Arbitrary`-API targets: free continuous fuzzing plus automatic crash dedup, minimization, bisection, and fix verification is a large chunk of the triage pipeline (Section 6) handed to you. The key adaptation: pepper the kernel with `debug_assert!` validity checks (manifoldness, closedness, Euler-Poincare) so OSS-Fuzz's crash detector also catches *correctness* violations, not just memory unsafety. This converts the silent-wrong-answer problem into a crashing-and-thus-found problem.

### Source 4.4: Corpus distillation and minimization theory

**Citation.** *Corpus Distillation for Effective Fuzzing: A Comparative Evaluation* (https://arxiv.org/pdf/1905.13055); ForAllSecure/Mayhem "Efficient Corpus Minimization" (set-cover) (https://www.mayhem.security/blog/efficient-corpus-minimization); LLVM `-merge=1` set-cover contribution.

**Technique.** Corpus minimization keeps the smallest subset of inputs that preserves total coverage. `afl-cmin` and libFuzzer `-merge=1` do greedy coverage-preserving reduction; the set-cover formulation finds a near-minimal cover (reported ~60% fewer files, ~50% fewer bytes than naive). Distillation also removes seeds with identical coverage behavior so fuzzing budget is not wasted re-exploring equivalent inputs.

**Limitations.** Code-coverage-preserving minimization can **discard geometrically distinct inputs that share code paths** (two different coincidence configurations might exercise the same branches), so blind minimization can throw away the very diversity a geometry corpus needs. Minimization is also only as good as the coverage metric.

**Kernel relevance.** Keel needs minimization to keep the corpus runnable in CI (Section 9), but must minimize against the **right coverage notion**. Use a *combined* key: code coverage **plus** a geometric-configuration fingerprint (number of coincident faces, presence of tangency, genus, smallest-feature-to-tolerance ratio). Minimize to preserve coverage of *that* combined space, so you don't collapse all coincidence cases into one. This is the "code coverage vs configuration-space coverage" distinction made operational.

---

## 5. Public datasets as corpus sources and their honest limits

Public datasets are the cheapest way to acquire a million pathological inputs, but each has a bias and a license, and confusing "ML training set" with "kernel stress corpus" is a trap.

### Source 5.1: Koch et al., "ABC: A Big CAD Model Dataset for Geometric Deep Learning"

**Citation.** S. Koch, A. Matveev, Z. Jiang, F. Williams, A. Artemov, E. Burnaev, M. Alexa, D. Zorin, D. Panozzo. *ABC: A Big CAD Model Dataset for Geometric Deep Learning.* CVPR 2019. https://arxiv.org/abs/1812.06216 ; dataset: https://deep-geometry.github.io/abc-dataset/

**Technique.** One million CAD models collected from Onshape's public documents, each an **explicitly parameterized B-rep** (curves and surfaces with ground-truth parameterization), distributed as STEP plus the native Onshape parametric format plus sampled meshes at multiple resolutions. Provides ground truth for differential quantities, patch segmentation, and feature detection. Models are real human-authored mechanical parts.

**Limitations.** **Licensing is per-model and heterogeneous** (Onshape public documents under various Creative Commons-ish terms); you cannot assume blanket reuse, which matters for redistributing a derived corpus. The data is "clean by construction" (it came from a working kernel, Onshape's), so it under-represents the *malformed* inputs an importer must survive: it is a **validity and feature corpus, not a malformed-input corpus**. Quality is uneven; many models are trivial. Sheer size (TBs) makes full ingestion non-trivial.

**Kernel relevance.** ABC is Keel's primary **STEP-import and B-rep-validity** corpus and the substrate for the pass-rate metric (Section 8): run import + validate + round-trip-export across all 1M and track the percentage that survives. Because ABC is clean, it tests "do we faithfully read real parts," not "do we survive garbage." For the malformed side, pair it with deliberately corrupted ABC variants (truncate, perturb references, inject invalid topology) and with Thingi10K. Ingest incrementally (sharded chunks, Section 9) rather than all at once, and **track per-model license** so any published derived corpus is redistributable.

### Source 5.2: Zhou and Jacobson, "Thingi10K," and the in-the-wild survey methodology

**Citation.** Q. Zhou, A. Jacobson. *Thingi10K: A Dataset of 10,000 3D-Printing Models.* arXiv:1605.04797. https://arxiv.org/abs/1605.04797

**Technique.** 10,000 real Thingiverse meshes, **deliberately curated to contain in-the-wild pathologies**: self-intersections, non-manifold edges/vertices, degenerate triangles, open boundaries, duplicate faces, wildly varying scale and complexity. The dataset's purpose is to be representative of *inputs encountered in the wild*, not clean benchmarks, and it ships rich per-model metadata (genus, manifoldness, number of components) so you can stratify and query.

**Limitations.** Mesh, not B-rep; no parametric surfaces, so it does not exercise NURBS SSI directly. It tests the mesh/import/repair path, not the analytic boolean path.

**Kernel relevance.** Thingi10K is Keel's **mesh-import, mesh-repair, and tessellation** stress corpus, and any mesh-boolean or mesh-to-B-rep path. Its greater value is **methodological** (see fTetWild next): the metadata stratification is the template for tagging Keel's own corpus by pathology, enabling "pass rate on high-genus models" or "pass rate on non-manifold inputs" breakdowns rather than one opaque number.

### Source 5.3: Hu et al., "Fast Tetrahedral Meshing in the Wild" (fTetWild) — the robustness-survey methodology

**Citation.** Y. Hu, T. Schneider, X. Gao, Q. Zhou, A. Jacobson, D. Zorin, D. Panozzo. *Fast Tetrahedral Meshing in the Wild.* ACM TOG 2020. arXiv:1908.03581. https://arxiv.org/abs/1908.03581

**Technique.** fTetWild runs its mesher over **all of Thingi10K** and reports a single robustness headline: it successfully meshed **98.7% of inputs in under 2 minutes and 100% within 32 minutes**, on a fixed cluster budget per model (8 threads, 128 GB, 24 h cap). The algorithm is engineered to **maintain a valid floating-point mesh at every stage** (never entering an unrepresentable intermediate), which is what makes "succeeds on essentially everything" achievable. The methodology is: define a wild corpus, run the operation on all of it, report success/validity as a percentage, and make that number the headline metric.

**Limitations.** "Success" is generous (produced *a* valid mesh, not necessarily the *best* mesh); the metric folds quality and robustness together unless separately reported. The corpus (Thingi10K) is mesh, so the methodology transfers more directly to Keel's mesh paths than to NURBS booleans without an equivalent B-rep wild corpus.

**Kernel relevance.** This is the **methodological keystone of this entire file** and the project's robustness thesis made measurable. Keel should adopt verbatim: pick a wild corpus (ABC + corrupted-ABC for B-rep, Thingi10K for mesh, plus the synthetic adversarial generators of Sections 1-4), run each operation across all of it, and report a **single headline pass-rate that must monotonically increase release over release**. The "always maintain a valid representation, never enter an unrepresentable intermediate state" design discipline is directly applicable to Keel's tolerant boolean engine and is *why* a high pass rate is reachable. Detail in Section 8.

### Source 5.4: Willis et al., "Fusion 360 Gallery" (and MFCAD/MFCAD++, the STEP zoos)

**Citation.** K. Willis, P. Jayaraman, J. Lambourne, H. Chu, Y. Pu et al. *Fusion 360 Gallery: A Dataset and Environment for Programmatic CAD Construction from Human Design Sequences.* 2021. https://arxiv.org/pdf/2010.02392 ; data: https://github.com/AutodeskAILab/Fusion360GalleryDataset . Related machining-feature corpora: MFCAD, MFCAD++.

**Technique.** Three linked datasets: a **reconstruction** set of sketch-and-extrude **construction sequences** (with a Python environment that replays them), a **segmentation** set of ~35,858 B-rep bodies (~390k faces) face-labeled by the originating modeling operation, and an **assembly** set with joints and contacts. Ships B-reps as both `.smt` (Autodesk Shape Manager native) and `.step`. The reconstruction environment is a programmatic CAD construction sandbox.

**Limitations.** Smaller than ABC; the reconstruction set is intentionally restricted to sketch-and-extrude to keep the ML task tractable. `.smt` is kernel-native to ASM and not directly useful to Keel; you use the `.step` exports. Like ABC, it is clean-by-construction.

**Kernel relevance.** The **reconstruction construction sequences** are the most useful piece: a corpus of *human-authored* feature histories Keel can replay through its own modeling API, exercising sketch/extrude/boolean and the persistent-naming/regeneration paths (`kernel/07`, `kernel/19`) on realistic operation orderings. The **face-operation labels** are a free oracle for feature-recognition and segmentation testing (`kernel/15`). The **assembly contacts** are a real source of coincidence/grazing configurations (mated parts touch exactly), feeding the coincidence-first generators. MFCAD/MFCAD++ provide labeled machining features for feature-recognition regression.

**Honest synthesis on synthetic-vs-real bias.** Real datasets (ABC, Fusion 360, Thingi10K) give *realistic distributions* but are *clean* (ABC/Fusion) or *mesh-only* (Thingi10K), and under-sample exact coincidence because real modelers' coincidences are *intended* and the source kernel already handled them. Synthetic adversarial generators (Sections 1-4) give *unrealistic distributions* but *targeted degeneracy* and *known answers*. **You need both**: real corpora for the pass-rate headline and import fidelity, synthetic generators for the targeted hardening of specific failure modes. Neither alone is sufficient; using only real data leaves the degenerate corners untested, and using only synthetic data overfits to your own idea of what is hard.

---

## 6. Failure triage and the named-regression discipline

A found failure is worth little until it is **minimized, deduplicated, and frozen as a permanent named test**. This pipeline is the mechanism by which the corpus accumulates and the robustness thesis compounds.

**The pipeline.**

1. *Capture.* Every failure (fuzz crash, proptest counterexample, differential disagreement, corpus pass-rate regression) is captured with a **deterministic, serializable reproducer**: the exact input plus the seed and the operation, serialized via Keel's deterministic format (`kernel/14`). Non-reproducible failures are worse than useless.
2. *Minimize/shrink.* Reduce to the smallest input that still fails. For proptest this is automatic shrinking (Section 3); for fuzz crashes use `afl-tmin` / libFuzzer minimization; for a failing solid, shrink the **construction sequence** (drop features until the failure disappears, keep the last failing version). The output is a minimal, human-comprehensible reproducer.
3. *Deduplicate.* Cluster failures so one root cause is not filed a thousand times. Standard methods: **stack-hash** (hash the top N frames of the crash/assert backtrace) and **coverage-based** clustering (two crashes with the same new-coverage signature are likely the same bug). OSS-Fuzz/ClusterFuzz do this automatically; replicate the stack-hash dedup locally for non-OSS-Fuzz failures.
4. *Freeze as a named regression.* The minimized reproducer becomes a **permanent, named test case** in the golden suite, named for the bug it represents (`regr_tangent_cylinders_coincident_axis`), with provenance (where found, when, root cause) recorded. This is the **"every bug ever found becomes a test" rule** and is the single most important discipline in this file. The golden suite runs on **every commit**, fast, forever.

### Source 6.1: Metamorphic testing as a triage and oracle survey (Chen et al.)

**Citation.** T. Y. Chen, F.-C. Kuo, H. Liu, P.-L. Poon, D. Towey, T. H. Tse, Z. Q. Zhou. *Metamorphic Testing: A Review of Challenges and Opportunities.* ACM Computing Surveys 51(1):4:1-4:27, 2018. https://dl.acm.org/doi/10.1145/3708521 (and the 1998 origin, Chen, Cheung, Yiu).

**Technique.** Surveys metamorphic relation (MR) identification, MR-based test generation, and integration with other techniques. Relevant to triage: MT both **alleviates the oracle problem** and is a **test-generation strategy** (each MR turns one test case into a follow-up case). For triage specifically, a metamorphic *violation* is a self-contained failing pair that needs no external ground truth, so it minimizes and reproduces cleanly.

**Limitations.** Identifying good MRs is the hard, human part; a weak MR finds nothing. Surveyed broadly across software, not geometry-specific.

**Kernel relevance.** Bridges to Section 7: metamorphic violations are an excellent **source** of triage cases because each one is already an oracle-free, self-checking reproducer. When a metamorphic relation fails, the failing input pair drops straight into the freeze-as-regression pipeline.

(Crash dedup tooling and minimization specifics cross-reference Sources 4.1-4.3; OSS-Fuzz provides this triage as a service.)

---

## 7. Metamorphic and oracle-free input generation (input side)

`kernel/35` covers oracles; here the concern is **manufacturing input pairs related by a metamorphic relation**, so correctness is checkable without any ground-truth answer. The generator emits not one input but a *pair* (or family) plus the relation their outputs must satisfy.

**Geometric metamorphic relations and their generators.**

- *Rigid-transform invariance.* Generate `A`, then `A' = R(A)` for a random rigid motion `R`. The boolean/mass-property/intersection result of `A'` must equal `R` applied to the result of `A`. Generator: emit `(A, R)`; relation: `op(R A) == R op(A)` up to tolerance. Catches frame-dependent bugs and is trivially automatable.
- *Knot-refinement invariance.* Generate a NURBS `A`, then `A'` = `A` with extra knots inserted (which by construction represents the **identical surface**). Every operation must produce the same result on `A` and `A'`. Generator: emit `(A, refine(A))`. This is a razor-sharp test of parameterization-independence and a classic SSI bug source.
- *Boolean commutativity/associativity.* `A union B` vs `B union A`; `(A union B) union C` vs `A union (B union C)`. Generator emits the operand set and the relation checks order-independence.
- *Degree elevation, reparameterization, control-point reordering*: all representation changes that preserve the geometric object; the result must be invariant.
- *Round-trip identity.* `import(export(A)) == A`; `A then undo == A` (cf. `kernel/14`, `kernel/07`).

The crucial property: these generators produce **input pairs whose correct relationship is known even though the correct answer is not**, which sidesteps the oracle problem entirely. Pair generation should be **biased toward the degenerate regimes of Section 1** (refine a *near-tangent* surface; transform a *coincident* configuration) so the metamorphic check fires exactly where bugs live.

### Source 7.1: Metamorphic relations for geometry/scientific software

**Citation.** Chen et al. 2018 survey (above) plus the metamorphic-testing literature on graphics and scientific computing (e.g. applications to mesh processing and numerical programs surveyed therein); Wikipedia overview https://en.wikipedia.org/wiki/Metamorphic_testing .

**Technique.** Catalogs relation patterns (additive, multiplicative, permutative, invertive, compositional) that map onto geometry: permutative = operand reordering; invertive = undo/round-trip; compositional = refine-then-operate equals operate-then-refine; multiplicative = scale invariance of mass properties (volume scales by `s^3`, area by `s^2`).

**Limitations.** Metamorphic testing detects *relative* inconsistency, not absolute wrongness: if an operation is wrong in a way that *respects* the relation (wrong but transform-equivariant), MT misses it. So MT complements, never replaces, known-answer testing (Section 10).

**Kernel relevance.** The relation catalog is a direct generator spec for Keel. Implement each as a property over generated input pairs; the scale-invariance multiplicative relations give a *free* cross-check on the mass-properties engine (`kernel/06`). Combine with known-answer constructions (Section 10) to cover the equivariant-but-wrong gap MT alone leaves.

---

## 8. The robustness pass-rate methodology

This is the headline metric and the operational form of "robustness is a corpus."

**Definition.** For an operation `O` and a corpus `C`, the pass rate is the fraction of `C` on which `O` produces a **valid result satisfying its oracles**. Adopt a three-way outcome (the fTetWild model, refined):

- *Pass*: operation completed and the result passes all applicable oracles (valid topology, closed, correct Euler-Poincare, satisfies any known-answer or metamorphic check).
- *Partial*: operation completed and the result is **valid but not provably correct** (e.g. valid solid but no oracle to confirm it is the *right* solid), or completed with a documented graceful degradation (cf. `kernel/29`, `kernel/30`).
- *Fail*: crash, hang (exceeded time budget), invalid result (non-manifold, open, inconsistent), or oracle violation.

**Reporting.** Report the pass/partial/fail breakdown **per operation and per pathology stratum** (using the Thingi10K-style metadata tags: high-genus, non-manifold, near-tangent, sub-tolerance feature), not one opaque global number, so regressions are localizable. Fix a **per-model resource budget** (time, memory) as fTetWild does, so "hang" is well-defined and runs are bounded.

**Tracking and gating.** The pass rate is the kernel's **headline robustness metric**, tracked release over release; it must **monotonically increase**. In CI, gate on it: a commit that drops the pass rate on any stratum below the prior baseline (beyond a noise band) fails the build. Combine with the golden suite (Section 6) as a hard gate: golden cases must be 100% pass, always.

**Cross-reference.** The methodology is from fTetWild (Source 5.3). The validity oracles that define "pass" live in `kernel/35`. The determinism that makes a pass-rate number reproducible run-to-run lives in `kernel/14`.

---

## 9. Performance and scale of the corpus

A million-case corpus is only useful if it can actually be run, repeatedly, deterministically.

**Storage and sharding.** Store the corpus content-addressed (hash-named) so dedup is automatic and provenance is stable. Shard by hash prefix for parallel CI. Keep heavy real datasets (ABC, multi-TB) out of the repo: reference them by manifest (URL + hash) and fetch/cache on CI workers, committing only the **minimized + synthetic-adversarial + golden** subsets that must run every commit.

**Tiered execution (the only way to afford it).**

- *Per-commit (seconds-minutes)*: unit tests, proptest with a modest case count, the full golden regression suite, the minimized crash corpus. Fast, deterministic, gating.
- *Nightly (hours)*: larger proptest budgets, differential-vs-OCCT over a sampled corpus slice, `cargo-fuzz` sessions, pass-rate over a representative sample.
- *Weekly/release (cluster)*: full pass-rate sweep over ABC + Thingi10K + corrupted variants, the headline number.

**Sampling and prioritization.** For the tiers that cannot run everything, sample to **maximize configuration-space coverage** (stratified by pathology tag), and prioritize inputs that *recently* flipped pass/fail or that sit near a known fragile boundary. This is corpus minimization (Source 4.4) applied to scheduling.

**Determinism and flakiness.** Reproducible corpus runs require **deterministic execution**: fixed PRNG seeds recorded with every case, no dependence on iteration order of hash maps, no concurrency-dependent floating-point reduction order (cf. `kernel/14`). A case that passes and fails nondeterministically is a **determinism bug to be fixed**, not a flaky test to be retried; quarantine it, file it, and fix the nondeterminism. Tolerance-comparison oracles must have **stable, documented tolerances** so a metamorphic/differential check does not flap with the wind.

---

## 10. Generating geometry with known answers

Metamorphic and differential testing detect *inconsistency*; they cannot catch an error that is consistent across all your relations. The cure is inputs whose **correct answer is known analytically**, so the generator emits **input-plus-expected-answer pairs**.

**Known-answer constructions.**

- *Primitives with closed-form mass properties.* Box, sphere, cylinder, cone, torus: volume, surface area, centroid, and inertia tensor are exact formulas. Generator emits the primitive plus its analytic mass properties; the interrogation engine (`kernel/06`) is checked to tolerance.
- *Surfaces of revolution and Pappus's theorem.* Volume and surface area of a revolved profile are given by Pappus's centroid theorems, so any revolved generator yields a known volume/area without modeling the solid in a second system.
- *Analytic intersection curves.* Sphere-plane = circle (known center/radius); cylinder-plane = ellipse/circle/line pair; two coaxial cylinders = circles; sphere-sphere = circle. Generator emits the two surfaces plus the **exact intersection curve**, checking the SSI against ground truth, including the near-tangent limit where the circle shrinks to a point.
- *Booleans with known results.* Box minus a smaller centered box = shell of known volume; union of two boxes sharing a face = known combined volume (and a known shared-face count, a coincidence-first check); a primitive minus a coaxial cylinder = known annular solid. The expected **topology counts** (V, E, F, genus) are as valuable as the expected volume.
- *Algebraic and symmetric cases.* Constructions with exact rational coordinates where predicate signs are exactly determinable, giving the exact-predicate path a ground truth (ties to Source 1.1 and the predicate work).

**Generator design.** Write each known-answer generator as a function returning `(geometry, expected_answer, tolerance)`. Sweep its parameters (Section 1's parametric families) so the same generator emits both the generic case and the degenerate limit, *both with known answers*: two boxes overlapping by depth `d -> 0` have a volume that is a known function of `d` all the way to `d = 0`, so the generator checks the boolean's behavior continuously into the coincident limit. This is the **highest-confidence** test type because it pins an absolute answer, and it is the only thing that catches the equivariant-but-wrong errors metamorphic testing misses.

**Limitations.** Known-answer geometry is necessarily *simple* (you must be able to derive the answer), so it under-covers the messy real-world distribution that ABC/Thingi10K provide. It is a *correctness floor*, not a robustness ceiling.

---

## Corpus engineering for Keel: synthesis

This file argues that the corpus is the kernel's robustness, and below is the concrete program that operationalizes that thesis at milestone M7b and beyond.

**The generators to build (in priority order).**

1. *Constructive grammar generator* (Section 2): a DeepCAD/Fusion-360-style sketch-extrude-boolean-fillet command interpreter against Keel's own API, producing valid-by-construction solids with known construction histories. This is the workhorse and also feeds persistent-naming/regeneration testing.
2. *Coincidence-first relational generator* (Section 1): builds the second operand from the first by face-copy, exact translation, reflection, and grid-snapping, so the corpus is biased toward exact and near-exact coincidence, which is the real-CAD common case the failure modes live in.
3. *Parametric degenerate-family generator* (Section 1): `config(t)` sweeps through `t*` on a logarithmic grid straddling each named degeneracy (tangency, coincidence, sliver, sub-tolerance, near-singular parameterization). A robust kernel must vary continuously across the family.
4. *Geometric-type PBT strategies with integrated, witness-directed shrinking* (Section 3): proptest strategies for knot vectors, control nets, and (via the grammar) solids, shrinking the **recipe** not the coordinates, biased by targeted-PBT scoring toward the degenerate witness.
5. *Stateful operation-sequence model* (Section 3): randomly generated, shrinkable command sequences checked after each step against cheap invariants (validity, Euler-Poincare, volume non-negativity).
6. *Arbitrary-derived structured fuzz decoders* (Section 4): `#[derive(Arbitrary)]` geometric-input decoders for `cargo-fuzz` over the operation API, with coincidence snapping in the decoder; plus byte-level `cargo-fuzz` over importers seeded from ABC/Thingi10K.
7. *Metamorphic pair generators* (Section 7): rigid-transform, knot-refinement, boolean-commutativity, scale-invariance, and round-trip pairs, biased toward degenerate regimes.
8. *Known-answer generators* (Section 10): primitives, surfaces of revolution (Pappus), analytic intersection curves, and known-result booleans, each emitting `(geometry, expected_answer, tolerance)` and sweepable into the degenerate limit.

**The dataset ingestion plan.** Reference ABC (1M B-reps) and Thingi10K (10k meshes) by hashed manifest, fetched and cached on CI workers, not committed. Track ABC per-model licenses so any published derived corpus is redistributable. Use ABC for the STEP-import/validity pass-rate headline and Fusion 360 Gallery reconstruction sequences for realistic feature-history replay; use Thingi10K for the mesh-path pass rate and as the stratification-metadata template. Manufacture a **corrupted-ABC** variant (truncated, perturbed references, injected invalid topology) to cover the malformed-input gap the clean datasets leave. Apply to **OSS-Fuzz** for free continuous fuzzing, crash dedup, bisection, and fix verification on importers and the Arbitrary-API targets, and instrument the kernel with `debug_assert!` validity checks so semantic errors surface as crashes the fuzzer can find.

**The triage-to-golden discipline.** Every failure from any source (fuzz, proptest, differential, metamorphic, pass-rate regression) goes through: deterministic capture (serialize input + seed, `kernel/14`) -> minimize/shrink (recipe shrinking; `afl-tmin`) -> deduplicate (stack-hash + coverage) -> **freeze as a permanent named regression** with provenance, named for its root cause. The golden suite runs on every commit and must be 100% green. This is the "every bug ever found becomes a test" rule, and it is the mechanism by which Keel's corpus compounds toward the incumbents' 500k-model regression suites.

**The pass-rate metric.** Adopt the fTetWild model as Keel's headline robustness number: per-operation, per-pathology-stratum pass/partial/fail over the wild corpus, under a fixed resource budget, monotonically increasing release over release, gated in CI so no commit may regress a stratum. Pair it with the always-maintain-a-valid-representation design discipline that makes a high pass rate reachable for the tolerant boolean engine.

**How this operationalizes the thesis.** "Robustness is a corpus" stops being a slogan and becomes: (a) generators that manufacture the breaking inputs incumbents accumulated over decades, coincidence-first; (b) public datasets that bootstrap realistic distribution for free; (c) a triage pipeline that converts every failure into a permanent test so the corpus only grows; (d) a single, gated, monotone pass-rate number that makes robustness *visible and non-regressable*. The corpus is the moat, the generators are how an open project fills it faster than a closed one, and the pass rate is the scoreboard.

---

## References

- Kettner, Mehlhorn, Pion, Schirra, Yap. Classroom Examples of Robustness Problems in Geometric Computations. Comp. Geom. 40(1):61-78, 2008. https://www.sciencedirect.com/science/article/pii/S0925772107000697 ; https://inria.hal.science/inria-00344310/
- Wu, Xiao, Zheng. DeepCAD: A Deep Generative Network for Computer-Aided Design Models. ICCV 2021. https://openaccess.thecvf.com/content/ICCV2021/papers/Wu_DeepCAD_A_Deep_Generative_Network_for_Computer-Aided_Design_Models_ICCV_2021_paper.pdf
- Seff, Ovadia, Zhou, Adams. SketchGraphs: A Large-Scale Dataset for Modeling Relational Geometry in CAD. ICML 2020 Workshop. https://github.com/PrincetonLIPS/SketchGraphs
- Para et al. SkexGen: Autoregressive Generation of CAD Construction Sequences with Disentangled Codebooks. ICML 2022. https://arxiv.org/pdf/2207.04632
- Brep2Seq. A dataset and hierarchical deep learning network for reconstruction and generation of CAD models. J. Comp. Design Eng. 11(1):110, 2024. https://academic.oup.com/jcde/article/11/1/110/7582276
- Claessen, Hughes. QuickCheck: A Lightweight Tool for Random Testing of Haskell Programs. ICFP 2000.
- MacIver et al. Hypothesis (integrated shrinking, targeted PBT, swarm testing). https://getcode.substack.com/p/property-based-testing-5-shrinking ; https://well-typed.com/blog/2023/04/falsify/ ; https://seelengrab.github.io/articles/
- Hughes. Experiences with QuickCheck: Testing the Hard Stuff and Staying Sane. 2016. https://www.cs.tufts.edu/~nr/cs257/archive/john-hughes/quviq-testing.pdf ; quickcheck-dynamic https://github.com/input-output-hk/quickcheck-dynamic
- Zalewski. American Fuzzy Lop (AFL). Fioraldi et al. AFL++. WOOT 2020. https://www.usenix.org/system/files/woot20-paper-fioraldi.pdf
- libFuzzer. LLVM. https://llvm.org/docs/LibFuzzer.html ; structure-aware fuzzing https://github.com/google/fuzzing/blob/master/docs/structure-aware-fuzzing.md
- OSS-Fuzz (Google). https://google.github.io/oss-fuzz/
- Corpus Distillation for Effective Fuzzing: A Comparative Evaluation. arXiv:1905.13055. https://arxiv.org/pdf/1905.13055 ; Efficient Corpus Minimization (set cover). https://www.mayhem.security/blog/efficient-corpus-minimization
- Koch et al. ABC: A Big CAD Model Dataset for Geometric Deep Learning. CVPR 2019. https://arxiv.org/abs/1812.06216 ; https://deep-geometry.github.io/abc-dataset/
- Zhou, Jacobson. Thingi10K: A Dataset of 10,000 3D-Printing Models. arXiv:1605.04797. https://arxiv.org/abs/1605.04797
- Hu, Schneider, Gao, Zhou, Jacobson, Zorin, Panozzo. Fast Tetrahedral Meshing in the Wild (fTetWild). ACM TOG 2020. arXiv:1908.03581. https://arxiv.org/abs/1908.03581
- Willis et al. Fusion 360 Gallery: A Dataset and Environment for Programmatic CAD Construction from Human Design Sequences. 2021. https://arxiv.org/pdf/2010.02392 ; https://github.com/AutodeskAILab/Fusion360GalleryDataset
- Chen, Kuo, Liu, Poon, Towey, Tse, Zhou. Metamorphic Testing: A Review of Challenges and Opportunities. ACM Comput. Surv. 51(1):4, 2018. https://dl.acm.org/doi/10.1145/3708521 ; Chen, Cheung, Yiu. Metamorphic Testing (1998). https://en.wikipedia.org/wiki/Metamorphic_testing

### Cross-references within the Keel research set

- `kernel/08-kernel-engineering`: broad testing survey (this file is the deep dive on generation/curation, not a repeat).
- `kernel/35`: correctness oracles and verification (the judging side; this file is the input side).
- `kernel/14`: determinism and serialization (reproducible corpus runs, deterministic reproducers).
- `kernel/07`, `kernel/19`: persistent naming and regeneration (construction-sequence replay corpora).
- `kernel/06`: interrogation and mass properties (known-answer targets).
- `kernel/15`: feature recognition (Fusion 360 / MFCAD labels as oracle).
- `kernel/24`: canonical geometry recovery (symmetric/planar generated nets).
- `kernel/29`, `kernel/30`: graceful degradation and imperfect-B-rep handling (the "partial" pass-rate outcome).
