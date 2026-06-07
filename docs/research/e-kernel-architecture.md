# Track E: Kernel Architecture Lessons from Existing Systems and Surveys

Research deliverable for the Keel B-rep geometry kernel. Style note: this document avoids em-dashes by rule; it uses commas, colons, and periods instead.

## 1. Executive Summary

Every successful production B-rep kernel descends from one of two early-1970s research lineages and converges on a small set of architectural commitments that Keel should treat as settled engineering, not open questions. The dominant kernel, Parasolid, traces directly to Ian Braid's 1974 BUILD system at Cambridge, through Romulus (1978, the first commercial b-rep modeler), to the 1988 C rewrite that added double-precision arithmetic and NURBS. ACIS (1989, Spatial) shares the same intellectual ancestry through the Cambridge group that founded Spatial, but committed early to an object-oriented C++ architecture with native non-manifold topology. The defining lesson from 50 years of accumulated experience is that **robustness is an architectural property, not a feature you add later**. Parasolid's reputation rests on "meticulous software engineering" and an early, deep commitment to tolerant modeling (per-entity tolerances on f64) and two-way data compatibility, not on any single clever algorithm.

The open-source and new-language landscape tells the cautionary half of the story. Open CASCADE (OCCT), the only mature open kernel, carries a 30-year reputation for fragile boolean operations and a heavy "shape healing" burden, precisely because robustness was retrofitted (fuzzy booleans, ShapeFix) rather than designed in. The Rust attempts (truck, Fornjot) are instructive: truck demonstrates that a clean modular NURBS+B-rep kernel is achievable in Rust, while Fornjot's multi-year struggle and Hanno Braun's published retrospectives reveal exactly where the hard problems live (boolean operations, and the geometry/topology coupling that Keel's design must get right from day one). C3D (2012, the newest commercial kernel) proves that a from-scratch competitive kernel is still possible in the modern era, and its author Nikolay Golovanov published the single most useful book-length architecture reference available.

For Keel specifically: the chosen arena/handle topology store with a coedge (radial/loop) hierarchy and Euler operators is the **mainstream, correct, and well-validated** choice for a manifold-first kernel with a NURBS boolean roadmap. The partial-entity structure is only worth adopting if non-manifold modeling is a first-class goal, which the roadmap suggests it is not initially. The single most important architectural recommendation from the new-kernel evidence is to **separate the geometry layer from the topology graph** so that topology can stay stable and immutable while geometry iterates, a lesson Fornjot learned painfully and retrofitted.

## 2. Annotated Key References

**Zou, Qiang. "A note on solid modeling: history, state of the art, future." arXiv:2302.14373 (2023), Journal of Graphics 43(6), 2022.**
URL: https://arxiv.org/abs/2302.14373
A recent single-author survey tracing solid modeling milestones, fundamental research problems, and three forward trends, culminating in a proposed shift from Computer-Aided to Computer-Automated Design. Useful as a current framing of which problems (robust booleans, representation conversion) remain genuinely open after five decades. Note: the full text is in Chinese.

**Shapiro, Vadim. "Solid Modeling." Handbook of Computer Aided Geometric Design, Elsevier, 2002, pp. 473-518.**
The canonical survey chapter. Presents solid modeling as a unified mathematical theory (point-set topology, r-sets, regularized boolean operations) rather than a grab-bag of algorithms. The right conceptual foundation for understanding why tolerant modeling is mathematically awkward: real kernels approximate ideal r-set semantics on floating point.

**Golovanov, Nikolay. "Geometric Modeling: The Mathematics of Shapes." 2014 (English edition, self-published / C3D Labs).**
URL: https://c3dlabs.com/company/book/
The closest thing to a published architecture manual for a modern commercial kernel. Written by the principal author of C3D, based on 20+ years building the kernel behind KOMPAS-3D. Covers curve/surface/solid construction algorithms, the data structures of geometric objects, and the interconnection principles between model elements. For Keel this is the highest-value single reference on how a contemporary kernel is actually structured internally.

**Lee, Sang Hun, and Kunwoo Lee. "Partial Entity Structure: A Compact Boundary Representation for Non-Manifold Geometric Modeling." J. Comput. Inf. Sci. Eng. 1(4), 2001, pp. 356-365 (also ACM Symposium on Solid Modeling and Applications, 2001).**
URL: https://asmedigitalcollection.asme.org/computingengineering/article/1/4/356/471622
Proposes the partial-entity structure, which halves the storage of Weiler's radial-edge structure while preserving full topological adjacency queries without loss of efficiency. The reference point for deciding whether Keel needs a non-manifold representation. Its existence is also evidence that radial-edge remains the baseline to beat for non-manifold work.

**Weiler, Kevin. "The Radial Edge Structure: A Topological Representation for Non-Manifold Geometric Boundary Modeling." Geometric Modeling for CAD Applications, North-Holland, 1988.**
The foundational non-manifold data structure, organizing edge-uses radially around an edge so that an edge shared by N faces is representable. ACIS-class kernels follow this lineage. Essential background for understanding what the coedge hierarchy generalizes to if non-manifold support is ever required.

**Braun, Hanno. Fornjot blog and GitHub (ongoing, 2021-2025).**
URLs: https://www.fornjot.app/blog/ , https://github.com/hannobraun/fornjot/issues/2116 , https://www.fornjot.app/blog/why-fornjot-is-using-boundary-representation/
The most candid primary-source developer retrospective on building a B-rep kernel from scratch in Rust. Two takeaways are load-bearing for Keel. First, the post "Why Fornjot is Using Boundary Representation" documents an explicit rejection of signed-distance / implicit representations after intensive work, because common operations "just don't result in a correct SDF" and meshing them is either low-fidelity or "crazy complicated." Second, issue #2116 ("Separate geometry from topology") is a direct, reasoned argument that topology should be an immutable ID-keyed object graph and geometry a separate layer that references topology by ID, not the reverse, to avoid regenerating object chains on every interactive edit.

**truck (ricosjp/truck), Rust CAD kernel.**
URLs: https://github.com/ricosjp/truck , https://deepwiki.com/ricosjp/truck
A working pure-Rust modular B-rep + NURBS kernel with boolean operations, defeaturing, topological healing, STEP I/O, and WebGPU/WASM targeting. Organized as interdependent crates from low-level primitives upward. The proof-of-existence that Keel's technology choices are viable, and a concrete reference for crate decomposition.

**Engineering.com. "Parasolid, D-Cubed and Siemens: The Heart of Your CAD Software Belongs to Another."**
URL: https://www.engineering.com/parasolid-d-cubed-and-siemens-the-heart-of-your-cad-software-belongs-to-another/
A credible secondary history confirming the lineage (Romulus 1978 to Parasolid v1.0 1988), the FORTRAN/single-precision to C/double-precision/NURBS transition, 30+ years of forward file compatibility, Siemens' attribution of dominance to engineering quality plus first-mover tolerant modeling and two-way data compatibility, and Convergent Modeling (mixed analytic + NURBS + facet B-rep).

**Hacker News thread on Parasolid (item 15392547), including a comment from a self-identified "Parasolid dev 1989-1995."**
URL: https://news.ycombinator.com/item?id=15392547
Confirms the BUILD (1974, Ian Braid PhD) to Romulus (FORTRAN) to Parasolid (C) chain from a primary participant, and the assessment that Parasolid is "very difficult to see being replaced as it is so entrenched."

**OCCT documentation: Boolean Operations, Shape Healing, STEP processor.**
URLs: https://dev.opencascade.org/doc/overview/html/specification__boolean_operations.html , https://dev.opencascade.org/about/shape-healing
Primary documentation candidly stating that boolean operations are "fundamentally not robust over the entire possible space of geometric objects" and that "there has never been a complete theory developed," especially for models with gaps at edges. The fuzzy-boolean and ShapeFix machinery is the retrofit. The STEP docs reveal that per-entity tolerances diverge from the file's nominal uncertainty, the practical face of tolerant modeling.

**Mattferraro.dev. "CADmium: A Local-First CAD Program Built for the Browser."**
URL: https://mattferraro.dev/posts/cadmium
A widely read practitioner essay characterizing Parasolid as feature-complete but enormous and expensive, OCCT as free but dated and fragile, and truck as the modern Rust option whose advantages (WASM, compiler-checked refactoring, memory safety enabling safe parallelism) are exactly Keel's thesis.

## 3. Per-Kernel Lessons

### Parasolid (Siemens, lineage BUILD 1974 -> Romulus 1978 -> Parasolid 1988)
**Copy:** A small, stable, procedural C-style API (the PK interface) that has preserved forward file compatibility for 30+ years. Tolerant modeling on double precision from early on (per-entity tolerances rather than one global epsilon). The session/partition/rollback model that makes operations transactional and undoable at the kernel level. The discipline of "meticulous software engineering" over cleverness. Convergent Modeling shows the long-term value of a topology layer that can carry either exact (analytic/NURBS) or faceted geometry behind a uniform B-rep interface.
**Avoid:** Nothing fundamental; the main caution is that Parasolid's robustness is the product of decades of accumulated test cases and bug fixes, which no new kernel can shortcut. Do not assume the architecture alone confers robustness.

### ACIS (Spatial / Dassault, 1989)
**Copy:** Native non-manifold topology from the start (radial-edge lineage) if and only if non-manifold modeling is a goal; doing it later is far harder. The object-oriented entity model with a clean separation between topology entities and the geometry they reference. Tolerant edges as an explicit entity concept.
**Avoid:** The C++ class-hierarchy-heavy design has aged less gracefully than Parasolid's procedural core; deep inheritance and pointer-rich entity graphs are exactly what Rust's ownership model resists. The ShapeManager fork (Autodesk's ACIS derivative) is a reminder that licensing and fork divergence are real risks for proprietary kernels, an argument for Keel being open source. ACIS is widely described as "fading" relative to Parasolid.

### Open CASCADE / OCCT (1990s, open source)
**Copy:** Very little of the architecture; much of the cautionary value. Its honest documentation about boolean non-robustness is a gift. The fuzzy-boolean idea (a user-supplied extra tolerance to absorb near-coincident geometry) is worth adopting as a deliberate, designed-in capability rather than a patch.
**Avoid:** Retrofitting robustness. OCCT's "shape healing" (ShapeFix) burden exists because imported and computed shapes routinely violate the kernel's own invariants, and the kernel cannot guarantee them by construction. The lesson for Keel: enforce topological and tolerance invariants at construction time (Euler-operator and validation checks) so that "healing" is rarely needed. Also avoid OCCT's reputation-damaging build and documentation ergonomics; modern Rust tooling is a structural advantage.

### C3D (C3D Labs / ASCON, public release 2012)
**Copy:** Proof that a competitive kernel is still buildable from scratch by a focused team. A uniform topology structure shared between exact B-rep and polygonal/faceted models (the same idea as Convergent Modeling). A massive automated regression corpus (C3D Labs cites a database of 500,000+ test models) as the real engine of robustness. Golovanov's published algorithm and data-structure descriptions give Keel a concrete internal blueprint.
**Avoid:** No specific anti-patterns surfaced; C3D's main limitation for reference purposes is that the deepest details live in a book and proprietary code rather than open source.

### Rust / new-language attempts (truck, Fornjot, Plasticity-as-consumer)
**Copy (truck):** Modular crate decomposition (primitives, geometry, topology, meshing, I/O as separate crates), pure-Rust NURBS, and WASM/WebGPU readiness. truck validates Keel's entire technology bet.
**Copy (Fornjot):** The hard-won architectural lesson to separate geometry from topology, with topology as an immutable, ID-keyed object graph and geometry as a separate layer referencing it by handle. This maps almost exactly onto an arena/handle design and is the strongest external endorsement of Keel's storage model. Also Fornjot's explicit, documented rejection of implicit/SDF representations for a precise engineering kernel.
**Avoid (Fornjot):** Its mainline stalled and was repeatedly rewritten partly because boolean operations are the gate everything else waits behind, and because early geometry/topology coupling forced expensive object regeneration on edits. Sequence the roadmap so the topology core and validation are rock-solid before booleans, and bake in the geometry/topology split from commit one rather than refactoring to it.
**Note (Plasticity):** Plasticity (Nick Kallen, 2023) does not build its own kernel; it licenses Parasolid, the same kernel as SolidWorks, NX, Solid Edge, and Onshape. The lesson is market reality: even an excellent solo developer building a modern modeler chose to buy the kernel rather than build it, because the robustness gap is enormous. Keel's value proposition must be openness and modern architecture, not an expectation of matching Parasolid robustness quickly.

## 4. Design Impact for Keel

### ADOPT
- **Arena/handle topology storage with a coedge (loop + radial) hierarchy and Euler operators.** This is the mainstream, validated choice for a manifold-first kernel and aligns with Parasolid/Romulus lineage. Handles map naturally to Rust ownership and sidestep the pointer-graph problems that age C++ kernels poorly.
- **Separate geometry layer from the topology graph.** Topology is an immutable, ID/handle-keyed graph; geometry is a parallel layer that references topology by handle. This is Fornjot's retrofitted lesson and should be designed in from the start. It enables interactive editing without regenerating object chains and lets multiple geometry backends (analytic, NURBS, later facet) coexist behind one topology.
- **Tolerant modeling on f64 with per-entity tolerances from day one.** Parasolid, ACIS, and OCCT all converge here. A single global epsilon is a dead end. Store a tolerance per vertex/edge as a first-class attribute.
- **Designed-in fuzzy/tolerant booleans.** Adopt OCCT's fuzzy-boolean concept (user/operation-supplied extra tolerance to absorb near-coincidence) as a planned capability, not a patch.
- **Construction-time invariant enforcement via Euler operators and a validation pass.** Make illegal topology unrepresentable or at least caught immediately, so Keel avoids OCCT's shape-healing burden. Mantyla's Euler-operator framework and Stroud's "Boundary Representation Modelling Techniques" (2006) are the implementation references.
- **A large automated regression corpus as the real robustness engine.** C3D's 500,000-model database and Parasolid's decades of accumulated cases show this is non-negotiable. Build the test harness early.
- **Modular crate decomposition** following truck (geometry, topology, meshing, I/O as separate crates).
- **A small, stable, procedural-feeling public API** in the spirit of the Parasolid PK interface, with long-term file-format compatibility as an explicit goal.

### AVOID
- **Implicit / SDF / function-representation as the primary modeling representation.** Fornjot's documented retrospective: common operations do not yield correct SDFs, and meshing is either low-fidelity or extremely complex. Keep implicit methods, if any, as an auxiliary, not the core.
- **Deep C++-style class inheritance hierarchies for entities.** They age badly and fight Rust ownership. Prefer flat handle-indexed arenas with enums/traits for geometry kinds.
- **A single global tolerance epsilon.** A known dead end across all three mature kernels.
- **Retrofitting robustness after the fact.** Do not plan to "add a healing pass later." Enforce invariants by construction.
- **Treating non-manifold support as free to defer-and-bolt-on later.** If it is ever wanted, the representation choice must be made consciously up front (see INVESTIGATE), because converting a manifold coedge structure to full non-manifold afterward is expensive.

### INVESTIGATE
- **Verdict on the topology structure: the chosen arena/handle + coedge hierarchy + Euler operators is correct for Keel's stated manifold-first, NURBS-boolean roadmap.** The partial-entity structure (Lee & Lee 2001) and Weiler's radial-edge are superior only when non-manifold modeling (an edge shared by 3+ faces, lamina, wire-in-solid) is a first-class requirement. Recommendation: build the manifold coedge core now, but keep the coedge abstraction general enough that an edge's radial cycle could later hold more than two coedges, which is the natural generalization path toward radial-edge / partial-entity semantics. If non-manifold becomes a real goal, prefer the partial-entity structure over radial-edge on the published 2x storage advantage.
- **Session / partition / rollback model.** Parasolid's transactional rollback is valuable but adds significant complexity. Investigate whether a persistent/immutable arena (cheap structural sharing, which the geometry/topology split already encourages) can deliver undo/rollback more idiomatically in Rust than an explicit logging scheme.
- **Convergent / mixed exact-and-facet geometry behind one topology** (Parasolid Convergent, C3D polygonal). Likely out of scope for v1 but the topology layer should not foreclose it.
- **Booleans as the gating risk.** Both Fornjot's stall and OCCT's reputation locate the deepest difficulty in boolean robustness on NURBS-bounded solids. Investigate the surface-surface intersection and tolerant-stitching strategy early, on paper, before committing the topology API, since boolean needs (coedge splitting, tolerant merging) shape the API.
- **Read Golovanov's "Geometric Modeling" in full** as the primary internal-architecture reference, since it is the only book-length account of a modern kernel's actual data structures and algorithms.

## 5. Gaps

- **No open primary source fully describes Parasolid's or ACIS's internal data structures.** The lineage, API philosophy, and tolerant-modeling stance are well documented, but the actual entity layout is proprietary. Keel must infer internals from the academic literature (Mantyla, Weiler, Stroud, Lee & Lee) and from open kernels (truck, OCCT).
- **The Zou 2023 survey and Golovanov's book were not read in full here** (the survey is in Chinese; the book is paywalled). Both should be obtained and read directly before finalizing the topology and tolerance design.
- **Quantitative robustness comparisons are scarce.** Claims that Parasolid is more robust than ACIS or OCCT are near-universal but rest on reputation and anecdote, not published benchmarks. There is no public, reproducible boolean-robustness benchmark suite, which is itself an opportunity for an open kernel like Keel to create.
- **C3D's tolerant-modeling and boolean algorithms** are described only at the marketing level in blogs; the substance is in Golovanov's book and proprietary code.
- **Fornjot's most recent (2024-2025) experiment outcomes** were only partially captured; the sponsor-update posts from that period should be mined for the final verdict on whether the geometry/topology separation succeeded in practice.
- **Smaller kernels (SMLib/TSNlib from Solid Modeling Solutions, KCM, ShapeManager)** surfaced only thinly. SMLib's grounding in the Patrikalakis-Maekawa "Shape Interrogation for Computer Aided Design and Manufacturing" body of work is worth a dedicated follow-up, as it is the most rigorous published treatment of NURBS intersection and tolerance, directly relevant to Keel's boolean roadmap.
