# Track F: Machine Learning for Geometric Computing and CAD Kernels

Research review supporting the design of Keel, an open-source B-rep solid modeling kernel in Rust. Scope: where machine learning can pay off inside a Parasolid-class kernel, evaluated through a strict predict-then-verify lens. Literature prioritized 2018-2025 (arXiv, SIGGRAPH/TOG, CAD/CAGD, SPM, CVPR).

Style note: this file deliberately avoids em-dashes.

## 1. Executive Summary

The central finding is unambiguous: no published neural method is accurate enough to replace an exact predicate, a tolerant intersection result, or a boolean classification decision inside a geometry kernel. Neural surface representations have a hard accuracy ceiling (residuals on the order of 1e-3 of bounding-box scale, plus systematic smoothing of sharp features), which is several orders of magnitude coarser than the 1e-9 to 1e-12 decisions a kernel must make. Any architecture that puts a network on the kernel's correctness-critical path is disqualified.

That said, ML has a real and underexploited role as an oracle, initializer, or heuristic that feeds a classical certifier. This is exactly the AlphaGeometry pattern: a neural component proposes, a symbolic or exact component disposes. For Keel the highest-value applications of this pattern are (a) neural initial guesses for Newton iteration in closest-point projection and SSI point tracing, where a good seed lands you inside the quadratic-convergence basin and the classical iteration then certifies the answer to machine precision; (b) learned heuristics that replace hand-tuned thresholds (subdivision depth, when to escalate to exact arithmetic, tolerance band selection), where a wrong heuristic costs performance but never correctness because the underlying algorithm still runs; and (c) ML-guided test generation and degeneracy prediction, which is pure upside because the generated cases are run against the real kernel and verified by its own invariants.

The B-rep deep learning literature (UV-Net, BRepNet, SB-GCN, the Fusion 360 Gallery and ABC datasets) is mature and production-relevant, but almost entirely for tasks that sit outside the kernel core: feature recognition, segmentation, retrieval, and generative modeling. It bears on Keel only at the import and healing boundary, and as a source of training data and topology priors.

Recommendation in one line: adopt ML as a seeding and heuristic layer with classical certification on top, invest experimental effort in Newton initialization and degeneracy prediction, and keep neural representations entirely out of the geometric core.

## 2. Annotated Key References

### Numerical acceleration and learned initialization

**Adaptive Coordinate-Wise Step Sizes for Quasi-Newton Methods: A Learning-to-Optimize Approach** (2024). arXiv:2412.00059. https://arxiv.org/abs/2412.00059. Uses LSTM-based networks to predict per-coordinate step sizes for BFGS from local iterate information, reporting convergence in roughly 40 iterations where baselines need 150 to 160. Takeaway: learned step-size control is real and gives multi-fold speedups, and crucially it does not change the fixed point of the iteration, so the converged result is still certifiable by the classical stopping criterion. This is the cleanest template for a kernel-safe learned accelerator.

**Learning the Step-size Policy for the Limited-Memory BFGS Method** (2021). arXiv:2010.01311. https://arxiv.org/pdf/2010.01311. Establishes that a small network mapping optimization state to step size generalizes across problem instances without per-problem tuning. Takeaway: the policy is cheap to evaluate and transferable, which matters for a kernel that must call projection millions of times.

**Automated Algorithm Selection on Continuous Black-Box Problems by Combining Exploratory Landscape Analysis and Machine Learning** (2017). arXiv:1711.08921. https://arxiv.org/pdf/1711.08921. Foundational for the algorithm-portfolio idea: cheap features of a problem instance predict which solver in a portfolio will win. Takeaway: directly transferable to choosing among SSI strategies (lattice/marching vs subdivision vs algebraic) per surface-pair instance.

### B-rep deep learning (datasets and architectures)

**UV-Net: Learning from Boundary Representations** (Jayaraman et al., CVPR 2021). arXiv:2006.10211. https://arxiv.org/abs/2006.10211. Encodes each face by sampling its surface on a regular UV grid and feeding the grid to a 2D CNN, with a graph network over the face adjacency. Takeaway: shows structured UV sampling beats point clouds for B-rep learning, and the UV-grid feature is a ready-made input encoding if Keel ever needs a learned classifier over its own faces (for example, degeneracy or feature prediction).

**BRepNet: A Topological Message Passing System for Solid Models** (Lambourne et al., CVPR 2021). https://www.research.autodesk.com/publications/brep-net/. Defines convolution kernels over oriented coedges using topological walks through the B-rep data structure. Takeaway: a principled way to do message passing on exactly the winged-edge/coedge topology Keel will implement, useful if a learned heuristic needs topological context rather than just local geometry.

**Fusion 360 Gallery: A Dataset and Environment for Programmatic CAD Construction from Human Design Sequences** (Willis, Jones et al., 2021). arXiv:2010.02392. https://arxiv.org/pdf/2010.02392. Provides B-rep, mesh, and point-cloud representations with face-level annotation by originating CAD operation, plus an assembly dataset with 154,468 parts and joint/contact data. Takeaway: the single most useful real-world labeled B-rep corpus for any Keel-side learning or, more importantly, as a test corpus for the kernel itself.

**ABC: A Big CAD Model Dataset for Geometric Deep Learning** (Koch et al., CVPR 2019). arXiv:1812.06216. https://arxiv.org/abs/1812.06216. One million CAD models with explicit parametric curves and surfaces, giving exact ground truth for normals, segmentation, and reconstruction. Takeaway: the canonical benchmark and a goldmine of real NURBS and analytic surfaces for stress-testing SSI and projection at scale.

**Geometric Deep Learning for Computer-Aided Design: A Survey** (Heidari and Iosifidis, 2024). arXiv:2402.17695. https://arxiv.org/abs/2402.17695. Comprehensive map of GDL-on-CAD methods. Takeaway: confirms the field concentrates on retrieval, synthesis, and reconstruction, with essentially nothing on kernel-internal numerics, which is the gap Keel's experiments would explore.

### Neural representations and reverse engineering to B-rep

**Point2CAD: Reverse Engineering CAD Models from 3D Point Clouds** (Liu et al., CVPR 2024). arXiv:2312.04962. https://arxiv.org/abs/2312.04962. Hybrid pipeline: neural segmentation, then per-face fitting that picks the lowest-error analytic primitive or, for freeform faces, a custom implicit neural representation (mixed SiLU/sinusoidal activations), then deterministic topology recovery by intersecting adjacent surfaces for edges and adjacent edges for corners. Reports freeform residual 0.002 to 0.003 and full-model surface F-score 0.727 on ABC. Takeaway: the architecture itself is the lesson for Keel: neural where you can tolerate approximation (segmentation, freeform proposal), classical and deterministic where correctness matters (primitive fit selection, edge/corner intersection). Explicitly notes sensitivity to segmentation quality and that the topology stage is deterministic, not learned.

**Neural Kernel Surface Reconstruction** (Huang, Gojcic et al., NVIDIA, CVPR 2023 Highlight). arXiv:2305.19590. https://arxiv.org/pdf/2305.19590. Reconstructs implicit surfaces from large noisy point clouds using compactly supported learned kernels and a sparse linear solve. Takeaway: best-in-class for point-cloud-to-surface, but the output is an implicit field, not a parametric B-rep, so it sits firmly in the import/scan-to-CAD bucket, not the kernel core.

**DeepCAD / SolidGen / BrepGen** generative B-rep family. BrepGen (Xu et al., SIGGRAPH 2024, ACM TOG, https://dl.acm.org/doi/10.1145/3658129) is a diffusion model emitting B-rep directly via a structured latent tree of faces, edges, and vertices. SolidGen (Jayaraman et al., ICLR 2024, https://www.research.autodesk.com/app/uploads/2024/02/SolidGen_Paper.pdf) is autoregressive over B-rep entities. Takeaway: impressive generation, but both are limited toward prismatic/simplified geometry and neither guarantees a watertight, tolerance-valid solid. For Keel they are a source of adversarial and near-degenerate test inputs, not a component.

### Accuracy ceilings and negative results

**FlatCAD: Fast Curvature Regularization of Neural SDFs for CAD Models** (2025). arXiv:2506.16627. https://arxiv.org/html/2506.16627v1. And **NeurCADRecon: Reconstructing CAD Surfaces by Enforcing Zero Gaussian Curvature** (Dong et al., 2024, arXiv:2404.13420, https://arxiv.org/pdf/2404.13420). Both document that neural SDFs blur sharp corners and warp developable regions, and both add curvature priors specifically to fight this. Takeaway: the negative result is the headline. Neural fields systematically violate the exact constraints (flatness, constant curvature, sharp creases) that define analytic CAD geometry, and even with heavy regularization they only approximate. This is the empirical basis for keeping neural representations out of Keel's decision points.

### Neuro-symbolic predict-then-verify

**AlphaGeometry: An Olympiad-level AI System for Geometry** (Trinh et al., Nature 2024). https://deepmind.google/blog/alphageometry-an-olympiad-level-ai-system-for-geometry/. A neural language model proposes auxiliary constructions; a symbolic deduction engine verifies and derives. Solves 25 of 30 olympiad problems. Takeaway: the architectural proof point for the entire strategy here. The network is allowed to be wrong because every proposal is checked by an exact engine that never is. This is precisely the contract Keel must impose on any ML component.

### ML-guided testing

**Coverage Guided, Property-Based Testing (FuzzChick)** (Lampropoulos et al., OOPSLA 2019). https://dl.acm.org/doi/pdf/10.1145/3360607. Combines QuickCheck-style property testing with AFL-style coverage feedback and synthesized type-aware mutators. Takeaway: directly applicable to fuzzing Keel's SSI and boolean code with structure-aware generators, where the property is a kernel invariant (Euler-Poincare, manifoldness, orientation consistency) and coverage drives exploration toward degenerate branches.

**A Coverage-Guided Fuzzing Method Using Reinforcement-Learning-Enabled Multi-Level Input Mutation** (IEEE 2024). https://ieeexplore.ieee.org/document/10580893/. Shows RL-selected mutation actions improve coverage per unit time. Takeaway: a learned mutator that prefers near-tangent, near-coincident, and small-feature configurations would concentrate test budget exactly where geometry kernels break.

## 3. Assessment by Kernel Subproblem

### Surface-surface intersection (SSI)

What ML can contribute: a neural initial-guess generator for the Newton iteration that refines each intersection point, and a learned predictor of starting points / tracing step size along the intersection curve. The published evidence (the quasi-Newton step-size papers, plus the general ray/surface initial-guess practice) shows good seeds cut iteration counts several-fold and avoid divergence on hard configurations. Algorithm selection across SSI strategies per surface-pair instance is also viable. What ML cannot contribute: the topology of the intersection set (number of branches, loops, singular points) cannot be trusted from a network. Loop detection and singular-point classification must remain exact, because a missed loop is a wrong solid. The pattern: network seeds points and step sizes, classical marching plus interval/algebraic certification owns the topology.

### Booleans

What ML can contribute: degeneracy prediction (flagging that a given face-face or edge-face configuration is near-tangent or near-coincident so the kernel preemptively escalates to higher precision or exact predicates), and algorithm/parameter selection. What ML cannot contribute: the in/out classification of any point or face. Classification is the correctness core and must be decided by exact predicates with consistent tie-breaking (symbolic perturbation). A neural classifier here would be catastrophic. The pattern: ML as an early-warning system that changes which certified path runs, never the classifier itself.

### Projection and root finding (closest point, inversion)

What ML can contribute: this is the single best fit for ML in the whole kernel. A network mapping (query point, surface descriptor) to an initial (u,v) guess lands Newton inside its quadratic basin, after which the classical iteration converges to machine precision and self-certifies (residual and first-order optimality checks). Learned root counting/isolation can act as an oracle that proposes how many roots and roughly where, with a classical exact root isolator (Descartes/Sturm or interval Newton) confirming. What ML cannot contribute: the final coordinates and the guarantee that all roots were found. Those come from the certifier. The pattern: predict the seed and the count, verify exactly.

### Tolerances

What ML can contribute: learned, context-dependent tolerance band selection and threshold setting (when two points are "the same," subdivision depth, arithmetic-filter cutoffs), tuned by Bayesian optimization against a test corpus. A wrong choice degrades robustness statistics but is caught and overridden by exact fallback. What ML cannot contribute: a tolerance value that is trusted without a consistency check. Tolerance management must stay globally consistent (the kernel's tolerant-modeling contract), so any learned value is a default that exact escalation can override. The pattern: ML tunes defaults, the consistency framework enforces correctness.

### Testing

What ML can contribute: the highest-leverage, lowest-risk application. Coverage-guided fuzzing with learned/structure-aware mutators and property-based testing with learned generators target the degenerate corners (tangency, coincidence, slivers, micro-features) where kernels fail. Because every generated case is executed against the real kernel and checked against exact invariants, ML errors cost nothing: a bad input is just a discarded or interesting test. The pattern: ML proposes inputs, the kernel and its invariants are the oracle.

### Import and healing

What ML can contribute: this is where the mature B-rep and reverse-engineering literature lives. Segmentation (ParSeNet/HPNet), primitive fitting (Point2CAD), and surface reconstruction (NKSR) can propose a B-rep from messy meshes or scans, and learned feature recognition (UV-Net, BRepNet) can guide healing of malformed imports. What ML cannot contribute: a guaranteed-valid solid. Every proposed face, edge, and stitch must pass the kernel's validity checks. The pattern: ML reconstructs a candidate, the kernel certifies and heals it.

## 4. Design Impact for Keel

### ADOPT (predict-then-verify, low risk, clear payoff)
- Neural initial-guess seeding for Newton in closest-point projection and surface inversion. Classical iteration certifies to machine precision. This is the safest, highest-confidence win.
- ML-guided / coverage-guided fuzzing and property-based test generation against exact kernel invariants. Zero correctness risk by construction.
- Learned step-size / subdivision-depth heuristics for marching and tracing, where the fixed point is unchanged and the classical stopping criterion certifies.

### INVESTIGATE (promising, needs an experiment to de-risk)
- Degeneracy/failure prediction for booleans and SSI to drive preemptive precision escalation.
- Algorithm-portfolio selection among SSI/boolean strategies per instance via cheap landscape features.
- Learned tolerance and arithmetic-filter threshold defaults tuned by Bayesian optimization, gated by exact fallback.
- Learned root counting as an oracle in front of an exact isolator.
- B-rep-from-mesh proposal (Point2CAD-style) for the import/healing path only.

### AVOID (correctness-fatal)
- Any neural surface/SDF as a geometric representation in the core. Accuracy ceiling and sharp-feature blur are disqualifying.
- Neural in/out boolean classification or any learned final geometric decision.
- Trusting network-predicted intersection topology (loops, branch count, singular points) without exact verification.
- A network that outputs a tolerance or coordinate that is used without an exact check.

### Ranked CLI-runnable experiments

1. **Newton seed network for closest-point projection.** Hypothesis: a small MLP mapping (query point, local surface descriptor such as a UV-grid sample a la UV-Net) to an initial (u,v) reduces Newton iterations and divergence rate versus the current control-polygon heuristic. Data: sample random points around ABC and Fusion 360 Gallery surfaces; ground-truth (u,v) from the classical projector. Model: small MLP or 1D-CNN on the UV-grid. Metric: median iterations to 1e-10 residual and divergence rate. Certification: the classical Newton step runs unchanged after seeding; result accepted only on residual and first-order optimality, so a bad seed only costs iterations.

2. **Coverage-guided fuzzer with a learned mutator for SSI/booleans.** Hypothesis: an RL or structure-aware mutator biased toward near-tangent/near-coincident configurations finds more invariant violations per CPU-hour than random property testing. Data: seed corpus from ABC plus generated primitive pairs. Model: RL mutator over geometric perturbation actions (FuzzChick/RL-fuzzing style). Metric: unique invariant-violation crashes and branch coverage per hour. Certification: the kernel's Euler-Poincare, manifoldness, and orientation invariants are the oracle; every case is run for real.

3. **Degeneracy predictor for precision escalation.** Hypothesis: a classifier on cheap features of a face-face or edge-face pair predicts near-degenerate cases, letting the kernel escalate to exact arithmetic only when needed and beating a static threshold on robustness-vs-speed. Data: label kernel runs by whether the exact path changed the outcome. Model: gradient-boosted trees or small GNN on local topology (BRepNet-style). Metric: failure-catch recall at fixed exact-arithmetic budget. Certification: a false negative is still caught because the exact predicate runs at every true decision point; the predictor only reorders when to escalate.

4. **Algorithm-portfolio selector for SSI strategy.** Hypothesis: landscape features of a surface pair predict which SSI method (subdivision, marching, algebraic) is fastest/most robust, beating a fixed default. Data: run all strategies on a corpus, record winners. Model: classifier per the Bischl/Kerschke landscape-analysis recipe. Metric: wall-clock and success rate versus single-strategy baseline. Certification: every strategy already produces a verified result, so misselection only costs time.

5. **Bayesian optimization of kernel thresholds.** Hypothesis: BO over tolerance bands, subdivision depth, and filter cutoffs against the test corpus dominates hand-tuned defaults on a robustness/performance Pareto front. Data: the kernel's own regression corpus. Model: Gaussian-process BO. Metric: corpus pass rate at fixed time budget. Certification: exact fallback overrides any tuned value that would produce an inconsistent decision.

6. **Learned root-count oracle for univariate/bivariate root finding.** Hypothesis: a network predicting root count and rough locations speeds an exact isolator by pruning the search. Data: synthetic polynomial/spline systems with known roots. Model: small regressor/classifier on coefficient features. Metric: isolator speedup at equal completeness. Certification: an exact isolator (interval Newton or Sturm/Descartes) confirms the count and locations, so a wrong prediction only loses the speedup.

## 5. Gaps

- No published work puts neural acceleration directly inside production NURBS SSI or kernel-grade projection; the step-size and seeding results come from the optimization literature, so transfer to splines is unproven and is exactly what experiments 1, 4, and 6 would establish.
- No standard benchmark exists for kernel-internal robustness (degenerate-case catch rate, escalation efficiency). The ABC and Fusion 360 datasets give geometry but not labeled degeneracy or kernel-decision ground truth; Keel may need to build this corpus, which is itself a contribution.
- The generative B-rep models (BrepGen, SolidGen) do not guarantee validity, and there is no published validity-certification layer bridging them to a real kernel. That bridge is open territory.
- Cost and latency of neural seeding inside a tight kernel inner loop are not characterized in the literature; a network call per projection may be too slow versus a cheap analytic seed, so experiment 1 must measure end-to-end wall clock, not just iteration counts.
- Tolerant-modeling-specific learned tolerance selection is essentially unstudied; the algorithm-selection and BO literature is generic and has not been applied to a consistent tolerance framework.
