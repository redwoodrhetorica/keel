# 29. Graceful Degradation and Best-Effort Modeling on Dirty Geometry

Research dossier supporting the design of **Keel**, an open-source B-rep solid modeling kernel in Rust with Parasolid-class ambition.

## Scope and motivation

The user's framing for this dossier is sharp and worth restating: Parasolid's single strongest, most-differentiating feature is not any one operation. It is that Parasolid **handles dirty geometry gracefully**. Bad input still gets a usable answer where nearly every other kernel either crashes or declines to attempt the operation. This dossier researches the *philosophy and architecture* of that graceful degradation, not the defect-by-defect repair recipes.

The distinction matters because graceful degradation is a **whole-system design stance**, not a feature you bolt on. It decides the shape of every operation's API (does it return a body, or a body plus a fault report?), the success/failure boundary (does an operation refuse, or widen tolerance and proceed?), the escalation structure (one algorithm, or a portfolio that cascades exact to tolerant to perturbed to meshed?), and the contract the caller signs (a returned body is "valid" by what definition, and how much was silently changed to get there?). Getting this stance right is what separates a kernel people trust on real-world imported junk from one that only models its own clean output.

This dossier deliberately does **not** re-cover material in sibling files: concrete defect-by-defect repair techniques (file 30), import healing pipelines and surface extension (kernel/13), tolerant boolean *mechanics* (track D), curved exact predicates (kernel/11), robust numerics and filtered arithmetic (a-robust-numerics), and mesh/implicit hybrid representation (kernel/09). Those are referenced as the substrate this dossier sits on. The subject here is the **graceful-degradation design itself**: best-effort semantics, fuzzy modes, partial results, robustness cascades, repair-in-the-loop, inverted-input correction, confidence reporting, the validate-after-degrade contract, and the mesh/implicit robustness floor.

Organization:

- **Part 1: The graceful-degradation philosophy** (best-effort vs all-or-nothing; the always-return-an-answer stance; the OCCT and CGAL contrasts)
- **Part 2: Tolerant and fuzzy operation modes** (how a tolerance changes the success boundary; the robustness-vs-silent-shape-change tradeoff)
- **Part 3: Partial results and best-effort returns** (operation status, per-entity fault reporting, the API shape of a knowingly-consumed degraded result)
- **Part 4: Robustness cascades and algorithm portfolios** (escalation tiers; controlled and symbolic perturbation as cascade rungs; never-panic engineering)
- **Part 5: Repair-in-the-loop** (in-operation healing vs pre-pass healing)
- **Part 6: Inverted and inconsistent input** (orientation, normals, negative volume, self-intersection)
- **Part 7: Quality and confidence reporting** (achieved-tolerance, "verify this" signaling)
- **Part 8: The risk side and bounding it** (validate-after-degrade; the non-negotiable topology invariant)
- **Part 9: Mesh/implicit fallback as the robustness floor**
- **Part 10: Reputation, corpora, and why this is hard to replicate**
- **Graceful-degradation doctrine for Keel** (synthesis)
- **References**

Each significant source gets a Citation / Mechanism / Limitations / Kernel relevance entry.

---

# PART 1: The graceful-degradation philosophy

## 1.1 Best-effort versus all-or-nothing operation semantics

The foundational design choice is binary at the top and graded underneath. An **all-or-nothing** operation has exactly two outcomes: it produces a fully valid result, or it fails and produces nothing. An **best-effort** operation has a third outcome and often more: it produces a result that is usable but imperfect, accompanied by enough metadata that the caller knows what it is holding. Parasolid's reputation rests on living in the second regime while still guaranteeing that whatever it returns is **topologically valid**, even when the geometry was salvaged.

### Source: Parasolid product positioning and tolerant-modeling reputation (Siemens / Tech Soft 3D / Grokipedia)

**Citation.** Siemens Digital Industries Software, *Parasolid* product pages, https://plm.sw.siemens.com/en-US/plm-components/parasolid/ and http://www.parasolid.com/ ; Tech Soft 3D, *Parasolid: The Leading Modeling Kernel*, https://www.techsoft3d.com/products/parasolid ; *Parasolid*, Grokipedia, https://grokipedia.com/page/Parasolid (accessed June 2026).

**Mechanism.** Across vendor and tertiary descriptions, Parasolid's differentiators cluster around robustness on imperfect data rather than raw feature count. The kernel supports **tolerant modeling**: tolerant edges and tolerant vertices that represent contact curves and contact points for surfaces that do not intersect cleanly or only partially intersect. This lets the data structure *store* a body whose geometry does not close exactly, by recording how far off it is, rather than rejecting it. Parasolid has also accreted defect-detection-and-fix functionality at the mesh layer: corrupt meshes, non-manifold meshes, vertices with bad normals, degenerate facets, self-intersecting meshes, meshes with slits, and disjoint meshes are detected and repaired. The throughline is that the kernel is engineered to **accept** geometry that other kernels reject, then either heal it or carry the imperfection forward in a controlled way. Its reputation among CAD/CAM developers is precisely "robustness and reliability," which in this domain is a euphemism for "does not fall over on customer data."

**Limitations.** Vendor and tertiary sources describe *that* Parasolid is robust, not *how*, in algorithmic detail. The deep mechanism (the regression corpus, the internal cascades) is proprietary and only visible indirectly through API surface and reputation. Tolerant modeling is not free: a tolerant body is geometrically fuzzier, and downstream operations must respect the larger tolerances or they reintroduce the error.

**Kernel relevance.** This is the north star. Keel's headline differentiator versus every existing open kernel should be the same: accept dirty input, return a valid answer. That means Keel's data model must support tolerant edges/vertices from day one (a local tolerance per entity, edges as tubes, vertices as spheres, per 2.x below), and Keel's operation semantics must be best-effort by default with all-or-nothing available as a strict mode for callers who prefer a hard failure.

### Source: the "always return an answer" stance versus OpenCASCADE's documented robustness gap

**Citation.** Open CASCADE Technology, *Boolean Operations* specification, https://dev.opencascade.org/doc/overview/html/specification__boolean_operations.html ; forum thread *Boolean operations: in search for a robust process*, https://dev.opencascade.org/content/boolean-operations-search-robust-process ; *opencascade and Boolean operations*, https://dev.opencascade.org/content/opencascade-and-boolean-operations (accessed June 2026).

**Mechanism.** OCCT is the natural foil because it is the only major open kernel and because its limitations are openly documented. The OCCT boolean spec contains an explicit **"Algorithm Limitations"** section enumerating cases the General Fuse Algorithm cannot handle robustly: pure self-interferences in the input, *acquired* self-interferences introduced by tolerance, and parametric-representation problems on certain surface types. In the forum, OCCT developers concede the gray zone directly: many warnings "are treated correctly inside and used just to draw the attention of the user," so it is "a bit strict to reject the shape after the warning is printed." Practitioners report the failure modes that define the gap: booleans that return a `TopoDS_Wire` with no faces, fuses of valid individual solids producing invalid compounds, and twisted operands returning "an invalid result that is not a solid, an incomplete boolean." The community consensus workaround is *not* "the kernel will handle it" but "pre-condition the input, pick a fuzzy value by hand, and check the result yourself." That is the opposite of graceful degradation: the burden of robustness is pushed onto the caller.

**Limitations.** OCCT has improved markedly (fuzzy booleans, gluing mode, history). The point is not that OCCT is bad but that its **default posture is all-or-nothing**, with tolerance handed to the user as a manual knob rather than managed by the kernel. The famous "fundamentally not robust" framing is a paraphrase of the community's lived experience and the spec's explicit limitations, not a verbatim vendor quote.

**Kernel relevance.** Keel must invert OCCT's default. Where OCCT says "here is a fuzzy knob, you tune it," Keel should say "the kernel will choose and escalate the tolerance, and tell you what it chose." The OCCT limitations list is also a free test plan: every case OCCT documents as a limitation is a case Keel should have a regression fixture for.

### Source: CGAL exact-but-brittle on real imperfect data

**Citation.** CGAL, *3D Boolean Operations on Nef Polyhedra* user manual, https://doc.cgal.org/latest/Nef_3/index.html ; Granados, Hachenberger, Hert, Kettner, Mehlhorn, Seel, *Boolean Operations on 3D Selective Nef Complexes* (Nef_3); plus practitioner comparison in S. Friedl, *Cork: A High Performance Library for Geometric Boolean/CSG Operations*, https://stephanfr.com/2016/03/21/cork-a-high-performance-library-for-geometric-booleancsg-operations/ (accessed June 2026).

**Mechanism.** CGAL's Nef_3 booleans use exact arithmetic (rational/extended kernels) and handle *all* degeneracies, producing provably correct results. This is the academically exact pole. But the correctness is conditional on the input being a clean, consistent, well-formed Nef polyhedron. Practitioners report that on real CAD-derived data, exact rational constructions can produce polyhedra "so broken they must be decomposed into polygon soup, repaired, and then reconstructed," and that double-precision input "fails with errors." Friedl's library comparison captures the polarity: "CGAL's numeric precision and stability is impeccable," yet it is "very slow" on moderately complex meshes, while the fast alternative (Cork) "is not robust" and "very sensitive to non-manifold meshes," assuming both inputs are 2-manifold, which "randomly sourced meshes are typically not." CGAL is correct-but-brittle: it gives a perfect answer on perfect input and refuses (or chokes) on imperfect input.

**Limitations.** Exactness solves the *numerical* robustness problem completely but does nothing for the *input-quality* robustness problem, which is the one that matters on dirty geometry. Exact arithmetic also imposes a speed penalty (reported factors up to 6x slower than ACIS, with cases where even ACIS fails) that rules it out as the always-on path in an interactive kernel.

**Kernel relevance.** This is the cautionary pole opposite OCCT. Keel should use exact predicates *inside* its cascade (the a-robust-numerics file covers this), but exactness alone is not graceful degradation. Keel must add the tolerant/best-effort layer *on top of* exact predicates so that imperfect input is accepted, not just so that clean input is computed correctly. The lesson: correctness and robustness-on-dirty-data are orthogonal axes, and Keel needs both.

---

# PART 2: Tolerant and fuzzy operation modes

## 2.1 How a tolerance changes the success/failure boundary

A fuzzy or tolerant mode is the simplest, most powerful graceful-degradation primitive: a user (or kernel) supplied tolerance that lets an operation succeed on imperfect input by widening the radius within which entities are treated as coincident, so near-misses snap together and the operation proceeds. It moves the success/failure boundary outward at the cost of allowing a controlled, bounded shape change.

### Source: OCCT fuzzy boolean (BOPAlgo fuzzy value), the one openly documented example

**Citation.** Open CASCADE Technology, *Boolean Operations* spec (fuzzy section), https://dev.opencascade.org/doc/overview/html/specification__boolean_operations.html ; `BOPAlgo_Options::SetFuzzyValue`, https://dev.opencascade.org/doc/refman/html/class_b_o_p_algo___options.html ; forum *Fuzzy Boolean Operations*, https://dev.opencascade.org/content/fuzzy-boolean-operations ; *how to guess the fuzzy value for boolean operations?*, https://dev.opencascade.org/content/how-guess-fuzzy-value-boolean-operations (accessed June 2026).

**Mechanism.** Fuzzy value is "an additional tolerance for the operation to detect touching or coinciding cases." Concretely, the algorithm widens the effective tolerance of every input shape by the fuzzy value before intersection, so entities that are within `fuzzy` of each other are treated as coincident. The documented purpose: fuzzy booleans are "useful on shapes with gaps or embeddings that result from modeling mistakes, the translating process, or import from other systems with loss of precision, where standard Boolean operations often give unsatisfactory results." Set via `SetFuzzyValue()`, default `Precision::Confusion()`. This is the **only openly documented** industrial fuzzy-boolean knob, which makes it the canonical reference example.

**Limitations.** OCCT's own users document the sharp edges of a *manually-chosen* fuzzy value. "It is not easy to choose a good fuzzy value." A fuzzy value of 1e-4 can produce an *invalid* shape while the same arguments with no fuzzy value produce a valid one. If the fuzzy value exceeds the length of some edges in the arguments, the operation collapses those edges and fails. The forum's hard-won heuristic is to group solids by minimum edge length and run independent operations with `Fuzzy = (smallest edge of group / 1.05)`. The deeper lesson: a single global fuzzy value is a blunt instrument. Too small and it does not bridge the gap; too large and it destroys small legitimate features. This is the **robustness-vs-silent-shape-change tradeoff** made concrete.

**Kernel relevance.** Keel should expose a fuzzy/tolerant boolean mode, but learn from OCCT's pain: do not make it a single global scalar the user must guess. Keel should (a) default the tolerance from the bodies' own stored tolerances, (b) scale it per-region by local feature size so it never exceeds a fraction of the shortest nearby edge, and (c) escalate it automatically within a bounded range rather than asking the caller to bisect by hand. The "smallest edge / 1.05" folk heuristic should be a built-in guardrail, not tribal knowledge.

### Source: ACIS tolerant modeling and tolerant booleans (Spatial)

**Citation.** Spatial Corp., *3D ACIS Modeler*, Kernel R10 Chapter 6 *Tolerant Modeling* (http://www-isl.ece.arizona.edu/ACIS-docs/PDF/KERN/06TMOD.PDF), and Spatial blog *Subtleties of B-rep Translation (Part 3): Why Healing Matters*, https://blog.spatial.com/3d-software-development-kits/subtleties-b-rep-translation-part-3-why-healing-matters ; *Healing in 3D interoperability*, https://blog.spatial.com/healing-in-3d-interoperability (accessed June 2026).

**Mechanism.** ACIS attaches **tolerance values to individual edges and vertices**, so that even when two edges do not intersect within `SPAresabs` (the model's resolution absolute), they can be "brought close enough to be useful." Geometry translated from another system is imprecise "due to the inherent limitations of the parent system" and "limitations of data transfer through neutral file formats," producing gaps and missing connectivity. ACIS's healing pipeline ends with an optional **"Make Tolerant Curves & Vertices"** step that allows "special handling of unhealed entities for Booleans, allowing inaccurate geometry to be tolerated." Critically for the API stance: "tolerances are maintained by the system and are updated after each operation with no user input necessary." So unlike OCCT's manual fuzzy value, ACIS *manages* the tolerance through the operation chain automatically. Tolerant entities are the explicit fallback for geometry that healing could not perfect: rather than fail, ACIS widens the local tolerance on the offending entity and proceeds.

**Limitations.** Tolerant entities propagate fuzziness: a body full of tolerant edges has many local regions where coincidence is only approximate, and successive operations can compound the looseness. Making everything tolerant is a last resort, not a first choice, because it weakens the geometric precision of the model.

**Kernel relevance.** ACIS's "make tolerant as the final fallback after healing fails" is exactly the escalation rung Keel needs between "heal it" and "give up." The automatic, no-user-input tolerance maintenance through operation chains is the *right* default and is what OCCT lacks. Keel should adopt per-entity local tolerance (the Jackson model, 2.2) and the rule that tolerances are kernel-managed and updated after every operation.

### Source: Jackson, boundary representation modelling with local tolerances

**Citation.** Jackson, D. J. (1995). Boundary representation modelling with local tolerances. *Proceedings of the Third ACM Symposium on Solid Modeling and Applications (SMA '95)*, 247-253. https://doi.org/10.1145/218013.218067 ; PDF at https://ftp.cs.wisc.edu/pub/users/prem/jackson-SM-95.pdf.

**Mechanism.** Jackson is the canonical academic formulation of the tolerant-B-rep idea that Parasolid and ACIS productized. Instead of a single global model tolerance, **each topological entity carries its own local tolerance**. An edge is conceptually a tube of radius equal to its tolerance; a vertex is a sphere. Two surfaces that do not intersect cleanly produce a tolerant edge whose tolerance is large enough to span the gap between them. Operations evaluate geometric relations (coincidence, on-edge, inside-face) against the *local* tolerance of the entities involved, not a global epsilon. This lets a model store and operate on geometry that is not exactly closed, because the imprecision is represented explicitly and locally rather than assumed away. The "edges as tubes, vertices as spheres" intuition (also surfaced in the Parasolid documentation) comes directly from this lineage: lowering precision makes the tubes thicker and the spheres larger.

**Limitations.** Local tolerances complicate every predicate: a point-on-edge test must account for the edge's tube radius, intersections become region-intersections, and the bookkeeping of how tolerances combine and update under operations is intricate. Done wrong it produces inconsistent classifications (a point inside one entity's tolerance but outside its neighbor's). The model is harder to validate than an exact one.

**Kernel relevance.** This is the data-model foundation for everything in this dossier. Keel's B-rep must carry a per-entity tolerance (defaulting to half the session precision for "exact" entities, per the Parasolid convention) and all predicates must be tolerance-aware. Without this, none of the best-effort behaviors are expressible: there is nowhere to *record* that an edge is only approximately closed.

### Source: Fang, Bruderlin & Zhu, tolerance-based intuitionistic robustness

**Citation.** Fang, S., Bruderlin, B., & Zhu, X. (1993). Robustness in solid modelling: a tolerance-based intuitionistic approach. *Computer-Aided Design, 25*(9), 567-576. https://doi.org/10.1016/0010-4485(93)90072-V ; related: Zhu, Fang, Bruderlin, *Obtaining robust Boolean set operations for manifold solids by avoiding and eliminating redundancy*, SMA 1993.

**Mechanism.** This is the theoretical companion to Jackson. Geometric relations (equal, less-than, coincident) are computed from a tolerance defined per geometric object, and the tolerances are **dynamically updated** during the computation to preserve the logical consistency of those relations (transitivity of equality, antisymmetry of order). The "intuitionistic" framing means a predicate can return not just true/false but an indeterminate band: within tolerance, the relation is *undecided* and the algorithm must commit to one branch consistently and propagate that commitment. The companion boolean paper shows how to get robust set operations on manifold solids by detecting and eliminating redundancy (duplicate/overlapping topology that ambiguous near-coincidences create).

**Limitations.** Maintaining global consistency of tolerance-based relations under updates is the hard part and the paper's scheme has scaling limits. Tolerance widening to preserve consistency can cascade (one relation forces a larger tolerance, which forces another), and the manifold-solid restriction excludes the non-manifold intermediate states real operations pass through.

**Kernel relevance.** The dynamic-tolerance-update discipline is exactly what ACIS means by "tolerances maintained and updated after each operation with no user input." Keel should implement predicates that return a three-valued result (definitely-true / definitely-false / within-tolerance-undecided) and, when undecided, commit consistently and record the commitment, so the same near-coincidence is never classified two different ways within one operation. That consistency is the difference between a salvaged-but-valid body and a self-contradictory one.

### Source: Qi & Shapiro, epsilon-solidity and epsilon-topological tolerant modeling

**Citation.** Qi, J., & Shapiro, V. (2006). epsilon-topological formulation of tolerant solid modeling. *Computer-Aided Design, 38*(4), 367-377. https://doi.org/10.1016/j.cad.2005.10.010 ; Qi, J., & Shapiro, V. (2006). Geometric Interoperability With Epsilon Solidity. *J. Comput. Inf. Sci. Eng., 6*(3), 213-220 ; Qi & Shapiro, *epsilon-Solidity in Geometric Data Translation*.

**Mechanism.** Qi and Shapiro give the most rigorous theory of what a tolerant solid *is*. Classical solid modeling relies on regular sets and presupposes exactness in both data and algorithms. Real systems involve approximations and varying numerical precisions. Their fix: replace point-set operations with **epsilon-topological operations** defined over finite-size neighborhoods, yielding **epsilon-regularity** and **epsilon-solid** as definitions that subsume the classical (exact) notions as the epsilon-to-zero special case. A body that is not a classical solid (it has gaps smaller than epsilon, or near-coincident faces) can still be a well-defined *epsilon-solid*. They then frame data translation and interoperability directly in these terms: a model imprecise to one system's epsilon is still a valid epsilon-solid, and translation is the problem of re-certifying epsilon-solidity under the target system's epsilon.

**Limitations.** It is a theory, not an implementation. The epsilon-operations are defined but the paper does not deliver a production algorithm portfolio. Choosing the right epsilon and proving an arbitrary imported body is an epsilon-solid for some usable epsilon is itself nontrivial.

**Kernel relevance.** This gives Keel the *definition* its tolerant mode certifies against. When Keel returns a best-effort body, the contract should be "this is a valid epsilon-solid for epsilon = X," where X is the achieved tolerance reported to the caller (Part 7). It converts the vague claim "the answer is usable" into a precise, checkable one. The epsilon-to-zero-subsumes-classical property means strict mode is just the epsilon-equals-resolution special case of the same machinery: one codebase, two postures.

## 2.2 The robustness-vs-silent-shape-change tradeoff

Every source above shares one warning: widening tolerance to succeed also **changes the shape**, often invisibly. A fuzzy union that bridges a 0.01mm gap has moved a face by up to 0.01mm and may have deleted a 0.005mm sliver feature that was real. The graceful-degradation design problem is not "how do I make the operation succeed" (widen tolerance until it does) but "how do I make it succeed while bounding and *reporting* the shape change so the caller can decide whether the salvage is acceptable." This reframing drives Parts 7 and 8.

---

# PART 3: Partial results and best-effort returns

## 3.1 Operation status: success / partial / warning / failure

The API-level expression of graceful degradation is a richer return type than a boolean. The operation does not return "did it work," it returns a *status with structure*: success, success-with-warnings, partial-result, or failure, plus a body (possibly degraded) and a list of per-entity faults.

### Source: Parasolid PK_BODY_check and the per-entity fault model

**Citation.** Siemens, Parasolid Functional Description ch. 9 *Checking* (http://www.q-solid.com/Parasolid_Docs/chapters/fd_chap.09.html), `PK_BODY_check` (http://www.q-solid.com/Parasolid_Docs/headers/pk_body_check.html), `PK_check_state_t` (http://www.q-solid.com/Parasolid_Docs_V35/headers/pk_check_state_t.html), `PK_FACE_check`.

**Mechanism.** Parasolid does not report validity as one boolean. `PK_BODY_check` returns a **list of faults**, each naming a specific entity and a specific fault enum. The state vocabulary is granular: `PK_EDGE_state_open_c` (edge not closed up topologically), `PK_EDGE_state_bad_vertex_c` (a vertex does not lie on the edge's curve within tolerance), `PK_EDGE_state_reversed_c`, `PK_EDGE_state_bad_order_c` (edges incorrectly ordered around a vertex), `PK_FACE_state_bad_vertex_c`, and many more. This is the key architectural point: the check is **per-entity and enumerated**, so a caller can distinguish "this body has three bad-vertex faults on these specific edges" from "this body is fundamentally not a solid." Faults can be diagnosed as fixable (re-attach curves so edges order correctly around the vertex) or fatal (internal tolerances grown too large, usually accompanied by other failures). Tolerant modeling interacts directly: an entity with no local precision set is treated as exact (precision = half session precision); checks evaluate against the entity's local tolerance.

**Limitations.** The fault list tells you *what* is wrong, not always *how* to fix it or *whether the body is still usable for your purpose*. Some faults are benign for one downstream use and fatal for another. The vocabulary is large and requires expertise to triage.

**Kernel relevance.** This is the model for Keel's result type. Keel operations should return a `result: Body` plus a `report: Vec<Fault>` where each `Fault` carries `{ entity_id, code: FaultCode, severity, achieved_tolerance }`. The enumerated, per-entity fault codes should mirror Parasolid's granularity (open edge, bad vertex, bad order, reversed, self-intersecting loop, inconsistent orientation). A caller can then make an informed decision instead of seeing a bare `Err`. This is the single most important API-shape lesson in the dossier.

### Source: the best-effort return API shape (synthesis across ACIS, OCCT, Parasolid)

**Citation.** Synthesis of: ACIS healing pipeline stages (Spatial blog, above); OCCT `BRepAlgoAPI_Check` and warning-not-rejection behavior (forum, above); Parasolid `PK_check_state_t` (above); CADfix DX automated-repair-with-report model, https://www.iti-global.com/interoperability-products/cadfix/cadfix-dx/.

**Mechanism.** Converging across vendors, the best-effort API has a consistent shape. (1) The operation **always returns a body** unless input is so degenerate there is nothing to return. (2) Alongside the body it returns a **status** distinguishing clean success from salvaged success. (3) It returns **per-entity diagnostics** tagging which entities are tolerant, which were healed, which remain faulty. (4) It exposes a **achieved-tolerance** or deviation figure so the caller knows how much was changed. OCCT's "warnings drawn to user attention rather than rejection" is a weak version of this; ACIS's automatic tolerance maintenance plus healing-stage reporting is a stronger version; CADfix's automated-repair-then-report-residual-defects is the interoperability-tool version. The unifying principle: **the caller consumes a degraded result knowingly**, with the kernel's honest assessment attached, rather than the kernel either hiding the degradation or refusing to produce anything.

**Limitations.** A rich result type is only useful if callers actually inspect it; many callers will ignore the report and consume the body blindly, which reintroduces the silent-shape-change risk at the application layer. The kernel cannot force good caller behavior.

**Kernel relevance.** Keel should make the degraded path *ergonomic but honest*: the default return is a `(Body, OpReport)` tuple, and `Body` should carry a queryable "fault-tagged" flag so even a careless caller who ignores the report can cheaply ask "is this body clean?" before relying on it. Fault tags should ride on the body itself (per-entity), not only in a side-channel report, so they survive serialization and downstream operations.

---

# PART 4: Robustness cascades and algorithm portfolios

## 4.1 Multi-strategy operations that escalate

The architecture that delivers graceful degradation at scale is a **portfolio**: an operation is not one algorithm but a ladder of strategies tried in order of preference, escalating from fast-and-exact toward slow-but-always-succeeds. Try exact; if that fails or is ambiguous, try tolerant; if that fails, perturb; if that fails, drop to a mesh. Each rung trades a little fidelity for a lot of robustness.

### Source: the filtered/cascaded-predicate pattern as the canonical escalation idiom

**Citation.** Synthesis of: filtered exact predicates (Shewchuk adaptive precision; CGAL `Exact_predicates_inexact_constructions_kernel`), as surfaced in the Cork/libigl comparison (https://stephanfr.com/2016/03/21/cork-a-high-performance-library-for-geometric-booleancsg-operations/) and libigl boolean docs (https://libigl.github.io/) ; Cherchi et al. interactive mesh booleans (below). See also a-robust-numerics dossier for the numeric mechanics.

**Mechanism.** The cascade idiom originates in *filtered predicates*: first attempt a fast floating-point evaluation with an error bound; if the result's sign is certain, return it; only if the sign is uncertain (near-degenerate) escalate to exact rational arithmetic, which is slow but definitive. The exact-as-backup-only structure means you pay for exactness only on the rare hard cases, getting floating-point speed in the common case with exact-arithmetic correctness everywhere. The same *shape* of cascade generalizes from a single predicate to a whole operation: attempt the operation with the cheap/strict strategy; on failure, escalate to the next more-robust strategy. The published mesh-boolean systems (libigl-on-CGAL for exact, Cork for fast-but-fragile) sit at different fixed points on this ladder; a graceful kernel implements the ladder itself.

**Limitations.** Cascades add latency (you sometimes run two or three strategies for one answer) and complexity (each rung must agree on the result format so the next rung can consume the same input). They also risk *inconsistency between rungs*: the exact rung and the tolerant rung can classify the same near-coincidence differently, producing a discontinuity in behavior as input crosses the escalation boundary.

**Kernel relevance.** Keel's boolean and intersection engines should be cascades, not monoliths. Tier 0: filtered-exact predicates (always on). Tier 1: exact-construction strict boolean. Tier 2: tolerant boolean with kernel-managed local tolerance (Part 2). Tier 3: controlled/symbolic perturbation (4.2). Tier 4: mesh fallback (Part 9). Each tier records which tier produced the answer so the report (Part 3) can say "this body required Tier 3," which is itself a confidence signal (Part 7).

### Source: controlled perturbation (Halperin, Mehlhorn, Funke, Klein) as an escalation rung

**Citation.** Halperin, D., & Shelton, C. R. (1998). A perturbation scheme for spherical arrangements with application to molecular modeling. *Computational Geometry, 10*(4). Mehlhorn, Osbild & Sagraloff; Funke, Klein, Mehlhorn, Schmitt (2005), *Controlled perturbation for Delaunay triangulations*; Mehlhorn et al., *Reliable and Efficient Computational Geometry via Controlled Perturbation*, ICALP 2006, https://people.mpi-inf.mpg.de/~mehlhorn/ftp/ControlledPerturbationGeneralStrategy.pdf ; Sharma & Yap, *Robust Geometric Computation*, Handbook of Discrete and Computational Geometry, ch. 45, https://www.csun.edu/~ctoth/Handbook/chap45.pdf.

**Mechanism.** Controlled perturbation solves the problem "not on the input given but on a nearby input." It moves the input points by a random delta small enough to be physically negligible but large enough to push every predicate that arises out of its degenerate/uncertain zone, so that fixed-precision floating-point evaluation is *guaranteed correct* with high probability. The "controlled" part is the analysis bounding how large delta must be to clear all predicate uncertainty given the working precision, and how small it can be to stay within the application's tolerance. Halperin introduced it for spherical arrangements (molecular modeling); it has since been extended to Delaunay triangulations, arrangements of circles, and Minkowski sums (Raab/Halperin controlled *linear* perturbation). It removes degeneracies *and* certifies correctness using only fast fixed-precision arithmetic, which is why it is attractive as a cascade rung between tolerant and mesh.

**Limitations.** It is a *perturbation*: the answer is for a nearby input, so it introduces a bounded shape change exactly like a fuzzy tolerance does, and must be reported as such. The analysis to bound delta is problem-specific and not trivially general. "High probability" is not certainty; a perturbation can occasionally fail and need to be retried with a larger delta, which costs more shape change.

**Kernel relevance.** Controlled perturbation is Keel's Tier 3: when exact and tolerant both fail (typically on genuinely degenerate configurations like exactly-coincident faces or triple points), perturb the input by a bounded, reported delta and retry. Because the perturbation magnitude is bounded and known, it slots cleanly into the achieved-tolerance reporting: "result computed on input perturbed by up to delta." This is graceful degradation with a quantified honesty cost.

### Source: symbolic perturbation / Simulation of Simplicity (Edelsbrunner & Mucke) as the degeneracy-handling rung

**Citation.** Edelsbrunner, H., & Mucke, E. P. (1990). Simulation of Simplicity: a technique to cope with degenerate cases in geometric algorithms. *ACM Transactions on Graphics, 9*(1), 66-104. https://doi.org/10.1145/77635.77639 ; arXiv math/9410209, https://arxiv.org/pdf/math/9410209. Practitioner notes: W. R. Franklin, *Implementing Simulation of Simplicity for Geometric Degeneracies*, https://wrfranklin.org/p/234-implementing-sos-2022.pdf.

**Mechanism.** Symbolic perturbation handles degeneracies *without actually moving the geometry*. SoS conceptually adds non-Archimedean infinitesimals of distinct orders to each coordinate, so that no predicate ever evaluates to exactly zero (no degenerate case ever arises), then modifies the predicates to simulate the effect of those infinitesimals using ordinary arithmetic. The algorithm proceeds as if every input were in general position. It "relieves programmers from providing a consistent treatment for every special case," making programs smaller and more robust. Unlike controlled perturbation, the shape does not change at all: the perturbation is symbolic, infinitesimal, and consistent, so the result is the limit as the perturbation goes to zero, resolved consistently.

**Limitations.** SoS gives *a* consistent answer to a degenerate case, but it is the answer for an arbitrary tie-breaking direction, which may not be the answer the application "wanted" (e.g., it commits to one side of an exactly-coincident-face case). It addresses *degeneracy* (exact ties), not *imprecision* (near-ties from dirty data); near-coincidences from dirty geometry are not exact zeros, so SoS alone does not bridge gaps. The bookkeeping of infinitesimal orders is intricate to implement correctly.

**Kernel relevance.** SoS is the right tool for the *exact-degeneracy* sub-problem inside Keel's exact tiers: when a predicate is genuinely zero (coincident, collinear, coplanar), SoS provides a consistent tie-break so the algorithm never branches on zero. It pairs with, rather than replaces, controlled perturbation: SoS for exact ties (no shape change), controlled perturbation for near-ties that need bridging (bounded shape change). Both rungs above feed the same consistency discipline from Part 2 (commit once, propagate).

### Source: never-panic engineering (Rust-specific robustness posture)

**Citation.** Design synthesis grounded in the failure reports above (OCCT returning wires-with-no-faces, twisted-operand invalid solids) and Rust error-handling conventions; cross-reference kernel/08 kernel-engineering and a-robust-numerics.

**Mechanism.** "Never panic" is the engineering corollary of "always return an answer." In Rust terms: a geometry kernel must never `panic!`, never `unwrap()` on a fallible geometric computation, never divide-by-near-zero without a guard, and never index out of bounds on degenerate topology. Every fallible step returns a `Result`; every numerically dangerous step has a guarded fallback. A panic is the worst possible graceful-degradation failure because it gives the caller *nothing*: no partial body, no fault report, just a stack unwind (or, across an FFI boundary, undefined behavior). The portfolio cascade is the structural realization of never-panic: instead of a strategy panicking on a case it cannot handle, it returns a typed failure that escalates to the next rung.

**Limitations.** Never-panic discipline is invasive: it touches every numeric line and requires fuzzing to enforce (the only way to prove no input panics is to throw millions of malformed inputs at it). It can mask bugs if "return an error" is used where "this should be impossible" is actually true.

**Kernel relevance.** This is a hard rule for Keel. No geometric operation panics on any input. The cascade plus typed `Result` plus a mesh-fallback floor (Part 9) means there is always *some* rung that returns *something*. Enforce it with a continuous fuzzing harness (the corpus of Part 10) whose pass criterion is "no panics, ever," separate from "correct answer."

---

# PART 5: Repair-in-the-loop

## 5.1 In-operation healing versus pre-pass healing

There are two places to heal: *before* an operation (a pre-pass that cleans the whole body once) and *during* an operation (snapping near-coincident entities and widening tolerance locally at the moment trouble is hit). Graceful kernels do both, and the in-operation kind is the subtler, more powerful one.

### Source: ACIS imprint/sew/heal-in-the-boolean-loop discipline (Spatial / Novedge)

**Citation.** Spatial healing blog (https://blog.spatial.com/healing-in-3d-interoperability) ; Novedge, *Design Software History: Boolean Modeling in CAD: CSG Origins, B-Rep Breaking Points, and Robustness Solutions*, https://novedge.com/blogs/design-news/design-software-history-boolean-modeling-in-cad-csg-origins-b-rep-breaking-points-and-robustness-solutions.

**Mechanism.** Novedge describes the productized in-loop repair pattern. Parasolid uses **symmetric imprinting** (consistent trimming on *both* operands so neither side has a one-sided trim artifact), **sliver mitigation** (area/angle thresholds that cooperate with documented tolerances to eliminate sub-threshold fragments *as they are created*), and the tolerant-modeling philosophy of "predictable behavior under controlled epsilon rather than pursuing fragile exactness." ACIS/ShapeManager normalized **imprint, sew, heal** as pre-Boolean conditioning *and* embedded boundary adjustment "within declared tolerances" into the operation itself. The key distinction from pre-pass healing: in-loop repair fixes problems that *the operation creates*, not just problems the input arrived with. A boolean that produces a sliver face does not return it as a defect; it absorbs the sliver during the operation using the same tolerance machinery. The Spatial blog frames healing as "not a single operation but a collection of complementary processes," some of which run continuously.

**Limitations.** In-loop repair can hide problems: a sliver silently absorbed is a small shape change the caller did not authorize. It also complicates the operation's logic enormously, because the operation must now reason about its own intermediate defects. The boundary between "this is a real feature" and "this is a sliver to absorb" is a threshold that is sometimes wrong.

**Kernel relevance.** Keel's boolean and local-ops engines should heal in-loop: snap near-coincident intersection vertices, absorb sub-resolution slivers, and widen local tolerance at the specific edge/vertex where intersection is failing, rather than failing the whole operation. This is distinct from the import healer (kernel/13), which is a pre-pass. The two share the same primitive operations (snap, merge, make-tolerant) but the in-loop healer applies them surgically and reports each application as a fault tag (Part 3), so absorbed slivers are not *silent*.

### Source: on-the-fly tolerance widening (ACIS auto-update + OCCT fuzzy-as-rescue)

**Citation.** ACIS Chapter 6 *Tolerant Modeling* (tolerances "updated after each operation with no user input"); OCCT fuzzy-value-as-rescue forum threads (above).

**Mechanism.** The mildest in-loop repair is simply *widening tolerance where the operation is stuck*. ACIS does this automatically: tolerances are maintained and updated after each operation. The OCCT pattern is the manual version: when a strict boolean fails, the user re-runs with a fuzzy value, which is on-the-fly tolerance widening done by hand at the API level. The graceful version makes this automatic and *local*: when an intersection fails to converge at a particular edge pair, the kernel widens the tolerance *at that edge pair* (not globally), retries, and records the widened tolerance as the achieved tolerance for that edge. This is the bridge between Part 2 (tolerant mode) and Part 4 (cascade): tolerance widening is the Tier-2 rung applied surgically rather than globally.

**Limitations.** Local widening can ripple: widening one edge's tolerance can make it overlap a neighbor, requiring the neighbor to widen too (the Fang-Bruderlin consistency cascade). Unbounded automatic widening is how you get wrong-but-plausible geometry, so the widening must be capped (per the "smallest edge / 1.05" guardrail).

**Kernel relevance.** Keel should widen tolerance *locally and boundedly* as an in-loop rescue, capping each widening at a fraction of local feature size, and tagging every widened entity. This gives the robustness of OCCT's fuzzy mode without the user having to guess a global value, and with a full audit trail of where fidelity was traded for success.

---

# PART 6: Inverted and inconsistent input

## 6.1 Wrong normals, flipped orientation, negative volume, self-intersection

A large fraction of dirty geometry is not *gappy* but *inconsistent*: face normals point the wrong way, adjacent faces disagree on which side is solid, the shell's signed volume comes out negative, or trim loops self-intersect. Graceful intake detects and auto-corrects these as part of accepting the body.

### Source: coherent orientation propagation and normal correction (MeshLab/Rhino/healing literature)

**Citation.** MeshLab *Reorient all faces coherently* filter (https://sourceforge.net/p/meshlab/discussion/499533/) ; Rhino *UnifyMeshNormals* (https://novedge.com/blogs/design-news/rhino-3d-tip-unify-mesh-normals-to-fix-shading-and-orientation-issues) ; CAD healing taxonomy for inconsistent orientations (Spatial blog and kernel/13 morphologic taxonomy).

**Mechanism.** The standard auto-correction for inconsistent orientation is **coherent orientation propagation**: build the face adjacency graph, pick a seed face, and BFS/DFS across shared edges, flipping each neighbor so that the shared edge is traversed in opposite directions by the two faces (the manifold-consistency condition). For a manifold shell this deterministically produces a coherent orientation; "if the mesh is manifold it usually does a good job." Once coherent, **global inside/outside** is fixed by computing the signed volume (or shooting a ray and counting crossings): if the signed volume is negative, the whole shell is inverted, so flip every normal. Self-intersection is detected by all-pairs (spatially indexed) face-face intersection tests; near-coincident self-intersections from dirty data are resolved by snapping, genuine ones are flagged. The healing literature classifies "inconsistent orientations" (adjacent faces disagreeing on the solid side) as a first-class morphologic defect with this standard repair.

**Limitations.** Coherent propagation is ambiguous at non-manifold edges (more than two faces meet, so "the other side" is undefined) and across disconnected shells (each component needs its own seed and its own inside/outside resolution). Signed-volume inside/outside fails on shells that are not closed (the very gappy bodies we are also trying to handle), so orientation repair and gap repair are coupled. "Auto-repair sometimes over-corrects, creating new problems," so correction must be verifiable.

**Kernel relevance.** Keel's intake should run coherent-orientation propagation per shell component, then signed-volume inside/outside per closed component, flipping as needed, and tag any face it flipped. For non-manifold junctions, defer to the non-manifold topology rules (kernel/02). Crucially, orientation repair must run *after* stitching (so shells are closed enough for signed volume to mean something) but its result must be re-validated, because over-correction is a known failure. This is graceful *intake*, the front door to every operation.

---

# PART 7: Quality and confidence reporting

## 7.1 Communicating how trustworthy a salvaged result is

A best-effort result is only safe if the caller knows *how* best-effort it was. The kernel must report achieved tolerance, deviation, and a "this was salvaged, verify it" signal.

### Source: achieved-tolerance and deviation reporting (Parasolid checking + epsilon-solidity)

**Citation.** Parasolid `PK_BODY_check` / `PK_check_state_t` (above, per-entity tolerance and fault state) ; Qi & Shapiro epsilon-solidity (above, epsilon as the reported fidelity figure) ; ACIS per-entity tolerance maintenance (above).

**Mechanism.** The confidence signal has three layers. (1) **Per-entity tolerance**: every tolerant edge/vertex carries the local tolerance it was relaxed to, so the body *is* its own deviation map; querying the maximum local tolerance gives the worst-case achieved fidelity. (2) **Provenance tags**: each entity records whether it is exact, healed, widened-in-loop, or perturbed (which cascade tier touched it). (3) **A body-level epsilon certificate**: by Qi-Shapiro, the body is a valid epsilon-solid for epsilon = max local tolerance, which is a single honest number the caller can compare against their application's tolerance budget. The combination lets a caller ask "is every tolerance under my 0.001mm budget?" and get a definite yes/no, and "which specific entities exceed it?" for targeted review.

**Limitations.** Achieved tolerance bounds *geometric* deviation but not *topological surprise*: a body can be within tolerance everywhere yet have merged two features the user wanted distinct. Reported tolerance is necessary but not sufficient for trust.

**Kernel relevance.** Keel should make every body queryable for `max_tolerance()` and per-entity `provenance()`, and should attach a body-level epsilon certificate to every best-effort result. The cascade tier that produced the answer (Part 4) is itself a coarse confidence grade: Tier 0-1 = trustworthy, Tier 2-3 = salvaged-verify, Tier 4 = mesh-fallback-degraded. Surface that grade prominently in the OpReport.

### Source: the "this answer was salvaged, verify it" signal (CADfix/Capvidia interoperability practice)

**Citation.** CADfix DX automated repair with residual-defect reporting, https://www.iti-global.com/interoperability-products/cadfix/cadfix-dx/ and https://www.cadinterop.com/en/your-needs/cad-repair-and-healing.html ; ITI CADfix DX 13 announcement, https://www.iti-global.com/resource-center/blog/unlocking-the-power-of-cadfix-dx-13-the-next-evolution-in-cad-data-exchange-and-processing/.

**Mechanism.** Interoperability tools, whose entire job is salvaging dirty CAD, have the most mature confidence-reporting practice. CADfix automates repair across 30+ native formats, "filling gaps, correcting surfaces, and adjusting tolerances without excessive manual intervention," but crucially **reports residual defects** that automation could not fix and exposes optional refinement for cases needing more. The workflow model is "automatic best-effort, then a defect report listing what remains," which is exactly the "salvaged, verify it" signal at the tool level. Healing is "intentionally implemented as a black-box operation enabled by default" yet exposes additional capabilities "when additional refinement is required," so the default is best-effort and the escape hatch is explicit.

**Limitations.** Tool-level reporting is coarser than kernel-level (it reports at the model/part granularity, not per-entity in the kernel's own type system). Residual-defect lists still require human judgment to act on.

**Kernel relevance.** Keel is the layer *below* tools like CADfix, so its reporting should be finer (per-entity, in-type) and feed upward. The doctrine to adopt: best-effort by default, residual faults always reported, refinement opt-in. Never silently succeed on a body you salvaged; always hand back the list of what you could not perfect.

---

# PART 8: The risk side and how to bound it

## 8.1 Validate-after-degrade and the non-negotiable topology invariant

Graceful degradation's failure mode is **wrong-but-plausible geometry**: a body that looks fine, passes a casual glance, and is subtly incorrect because tolerance widening merged or moved something. The discipline that bounds this risk is: validate after every best-effort operation, and hold certain invariants as non-negotiable even on a degraded result.

### Source: validity-after-best-effort and the topology-is-non-negotiable principle (synthesis)

**Citation.** Synthesis of: Parasolid `PK_BODY_check` as a *post*-operation gate (above); OCCT `BRepAlgoAPI_Check` recommended "only if warnings appear" (forum, above); Hoffmann, *The Problems of Accuracy and Robustness in Geometric Computation*, IEEE Computer 22(3), 1989, https://www.cs.purdue.edu/cgvlab/www/resources/papers/Hoffmann-IEEE-1989-The_problems_of_accuracy_and_robustness_in_geo_computation.pdf ; *Conversion to watertight B-rep* (USPTO 11,886,165) for the watertightness-as-invariant framing.

**Mechanism.** Hoffmann's foundational survey frames the core danger: floating-point predicate errors cause *combinatorial* (topological) errors, and a topologically inconsistent structure is far worse than a geometrically inaccurate one because subsequent algorithms branch on topology and can crash or loop. This motivates the central invariant: **geometry may be salvaged (moved, widened, approximated) but topology must remain valid**. A best-effort boolean may return a body whose faces are 0.01mm off, but it must *never* return a body whose shell is non-orientable, whose edges reference nonexistent vertices, or whose loops do not close topologically. The enforcement mechanism is **validate-after-degrade**: every best-effort operation runs a topology check (Parasolid-style `PK_BODY_check` over the result) before returning, and if topology is invalid, the operation does *not* return that body. It either repairs the topology, escalates to the next cascade tier, or (last resort) drops to the mesh floor (Part 9) which is topologically trivially valid. Geometric faults are reported and allowed; topological faults are blocked. The watertightness/manifoldness of the result is treated as a hard postcondition, not a best-effort target.

**Limitations.** A full topology check after every operation costs time; OCCT explicitly recommends checking "only if warnings appear" to avoid the cost. There is a tension between the never-panic always-return stance and the never-return-invalid-topology stance: if every cascade tier produces invalid topology, *something* must give, which is why the mesh floor exists as the always-valid backstop. Validation catches *invalidity*, not *wrongness*: a body can be perfectly valid and still be the wrong answer.

**Kernel relevance.** This is Keel's central safety contract, stated as two rules. **Rule 1 (geometry is negotiable):** best-effort operations may move geometry within reported tolerance and must report how much. **Rule 2 (topology is non-negotiable):** Keel never returns a body with invalid topology; validity (orientability, closed loops, consistent incidence, watertightness for solids) is a hard postcondition enforced by a post-operation check, and any operation that cannot achieve it escalates or drops to the mesh floor rather than returning invalid topology. Make the post-operation topology check cheap enough to always run (incremental, only over touched entities) so it is never skipped. This pairing, salvageable geometry over inviolable topology, is the precise technical content of "graceful degradation done safely."

---

# PART 9: Mesh/implicit fallback as the robustness floor

## 9.1 When B-rep genuinely cannot proceed, drop to mesh or SDF

The ultimate graceful degradation: when no B-rep tier can produce a valid body, fall back to a representation that is *trivially* always-valid, a triangle mesh or a signed-distance field, so that *something* watertight is always returned. This is the floor beneath the cascade.

### Source: robust mesh booleans as the always-valid floor (Cherchi et al.)

**Citation.** Cherchi, G., Pellacini, F., Attene, M., & Livesu, M. (2022). Interactive and Robust Mesh Booleans. *ACM Transactions on Graphics (SIGGRAPH Asia), 41*(6). https://doi.org/10.1145/3550454.3555460 ; arXiv:2205.14151, https://arxiv.org/pdf/2205.14151 ; context: Zhou et al., *Mesh Arrangements for Solid Geometry* (libigl), and Barki/Cazals exact mesh booleans.

**Mechanism.** Robust mesh-boolean research delivers the property B-rep booleans cannot guarantee: given *arbitrary* triangle soups (even self-intersecting, non-manifold, or inconsistently-oriented), produce a valid, watertight output mesh. Cherchi et al. achieve interactive speed by combining exact arithmetic (for the arrangement/intersection step, where robustness is essential) with floating-point (for the cheap steps), the same filtered-cascade idiom from Part 4, applied at mesh granularity. The paper notes the fidelity tradeoff explicitly: converting vertex coordinates to triangle-plane coefficients is "not exact and the result typically needs to be repaired," and in *interactive* B-rep modeling "approximations cannot be tolerated because approximation would quickly accumulate," which is exactly why mesh is a *fallback floor* and not the primary representation. But as a floor it is unbeatable: a mesh boolean essentially always returns a topologically valid watertight result, where a B-rep boolean on the same dirty input might return nothing.

**Limitations.** Mesh fallback loses the exact analytic representation: a cylinder becomes faceted, parametric editability is gone, and the result is an approximation whose fidelity depends on tessellation density. Recovering analytic B-rep from the mesh (kernel/24 canonical recovery) is lossy and not always possible. So the floor returns *something valid* but *degraded in kind*, the largest possible fidelity sacrifice.

**Kernel relevance.** This is Keel's Tier 4, the robustness floor that makes "always return an answer" literally true. When all B-rep tiers fail to produce valid topology, Keel tessellates the operands, runs a robust mesh boolean (exact-arithmetic arrangement, always-watertight output), and returns the mesh result tagged `representation: mesh, fidelity: degraded`. Tie this to kernel/09 (mesh/implicit hybrid) for the representation plumbing, but frame it here as the *last rung of the robustness cascade*, the guarantee that the cascade never bottoms out in failure.

### Source: implicit/SDF and volumetric wrapping as an alternative floor (Ansys Fluent wrap)

**Citation.** Ansys, *New Fluent Meshing Workflow Quickly Wraps Non-Watertight, Dirty Geometries*, https://www.ansys.com/blog/fluent-meshing-workflow-wraps-non-watertight-geometries ; general SDF/level-set floor (cross-reference kernel/09).

**Mechanism.** The simulation world has the most aggressive dirty-geometry floor: **volumetric wrapping**. Rather than repair the B-rep, the Fluent workflow *wraps* a non-watertight, dirty geometry by extracting a clean watertight surface from a volumetric/level-set representation of the input, ignoring the input's topological defects entirely. Conceptually: voxelize or compute an SDF of the dirty input, then extract an iso-surface, which is watertight *by construction* regardless of how broken the input was. This is graceful degradation taken to its logical end: the result is guaranteed valid because it is *reconstructed* from a representation in which invalidity cannot exist. SDF/implicit representations share this property: a level set is always a valid closed manifold at a regular iso-value.

**Limitations.** Wrapping/SDF-extraction is the *most* lossy floor: it discards the original topology entirely, fattens or shrinks by the voxel/band resolution, can bridge gaps that should stay open and close pockets that should stay hollow, and produces dense meshes. It answers "give me *something* watertight" but not "preserve the design." Sharp features are rounded at the resolution scale.

**Kernel relevance.** SDF-wrap is Keel's Tier 5, below even mesh booleans, for input so broken that even a robust mesh boolean cannot interpret it (e.g., wildly self-intersecting soup with no coherent interior). Convert to SDF, extract iso-surface, return as a degraded watertight mesh with a loud `fidelity: wrapped` tag. It should almost never fire, but its existence is what lets Keel *promise* a valid answer on literally any input. The two floors (mesh boolean, then SDF wrap) plus the never-panic rule make "always return a valid answer" a total guarantee, not an aspiration.

---

# PART 10: Reputation, corpora, and why this is hard to replicate

## 10.1 The regression corpus is the actual robustness engine

The uncomfortable truth across every source: graceful degradation is not primarily an *algorithm*, it is an *organizational asset*. The thing that makes Parasolid and ACIS robust on dirty data is decades of accumulated regression models and the engineering culture that fixes every reported failure into a permanent test.

### Source: C3D Labs 500k-model regression corpus

**Citation.** C3D Labs, *C3D Modeler* product page and QA description, https://c3dlabs.com/products/c3d-toolkit/modeler/.

**Mechanism.** C3D Labs (the only kernel vendor that publishes its QA scale) tests its solid/surface construction algorithms against "a specially created database containing over **500,000 3D models**." The QA stack is daily automated kernel compilation plus model-rebuild testing, flat-projection (HLR) testing, and data-conversion testing, alongside static analysis, unit tests, performance tests, and automated regression. The corpus *is* the robustness mechanism: every model that ever exposed a failure becomes a permanent fixture, so the kernel's robustness ratchets monotonically upward and never regresses on a previously-seen defect class. This is why a mature kernel handles dirty geometry the new kernel cannot: it has *seen* hundreds of thousands of dirty bodies and been hardened against each.

**Limitations.** A corpus protects against *seen* defect classes; novel dirty inputs can still fail. Building a corpus this large takes decades and a large user base feeding failures back. It is a moat precisely because it cannot be shortcut.

**Kernel relevance.** This reframes Keel's entire robustness strategy. Keel's graceful-degradation reputation will be built not by clever algorithms alone but by a **growing regression corpus** where every dirty model that ever broke Keel becomes a permanent test with a pass criterion of "no panic, valid topology out, reported tolerance within budget." Start the corpus on day one. Seed it from open STEP/IGES repositories and every OCCT-documented limitation case. Make corpus growth a first-class project metric.

### Source: the socio-technical robustness argument and the open-kernel gap (Novedge + HN)

**Citation.** Novedge robustness history (above) ; Hacker News threads on Parasolid/ACIS vs OpenCASCADE, https://news.ycombinator.com/item?id=31474409 and https://news.ycombinator.com/item?id=11536518 and https://news.ycombinator.com/item?id=15392547.

**Mechanism.** Novedge states the thesis directly: "Robustness is thus no longer just a feature of code; it is a property of the organization's test data, its feedback apparatus." It describes modern robustness as a "socio-technical alignment" of nightly CI, telemetry from cloud platforms (Onshape's global model telemetry feeding Parasolid failure clusters), and community test contributions. The HN commentary supplies the blunt market verdict: Parasolid and ACIS are "light years ahead of OpenCascade in capability and stability," and "the current state of open-source CAD is very sad." The gap is not that the open kernel's developers are less clever; it is that they lack the failure corpus and the feedback loop. Practitioner threads ("I worked on a project that used OpenCASCADE, and our conclusion was...") consistently attribute the gap to robustness on real data, not feature lists.

**Limitations.** HN commentary is anecdotal and dated (the open kernels improve continuously). The socio-technical argument, while true, is discouraging for a new entrant: it implies robustness cannot be bought, only accumulated.

**Kernel relevance.** The strategic lesson for Keel is to *design for corpus growth from the start*: deterministic operations (kernel/14) so failures reproduce, fault-tagged results (Part 3) so failures self-describe, and a never-panic harness (Part 4) so the fuzzing corpus can run unattended. Keel's open-source nature is an advantage here that Parasolid lacks: a public corpus that every user contributes failing models to could accumulate faster than a proprietary one, if the contribution path is frictionless. The moat is real, but it is the *one* moat an open project can plausibly out-build, given enough users and a disciplined feedback loop.

---

# Graceful-degradation doctrine for Keel

Synthesizing the dossier into Keel's design commitments.

### The operation-semantics stance

- **Best-effort by default, strict on request.** Every operation defaults to best-effort (return the most usable valid body it can) and accepts a `strict` flag that switches to all-or-nothing (return a hard error rather than a salvaged body) for callers who want it. The two postures are the epsilon-equals-resolution and epsilon-greater-than-resolution cases of one tolerant-modeling engine (Qi-Shapiro), not two codebases.
- **Always return an answer, never panic.** No geometric operation panics on any input. The cascade plus the mesh/SDF floor guarantees a valid result exists for every input. Enforced by a continuous fuzzing harness whose pass criterion is "no panic, valid topology out."

### The modes Keel exposes

1. **Strict mode** (all-or-nothing, exact): for clean input where the caller wants a hard failure on any imperfection.
2. **Tolerant mode** (per-entity local tolerance, kernel-managed): the default. Tolerant edges as tubes, tolerant vertices as spheres (Jackson), tolerances updated after every operation with no user input (ACIS), three-valued tolerance-aware predicates with consistent tie-break commitment (Fang-Bruderlin).
3. **Fuzzy boolean mode** (caller-hinted tolerance, kernel-bounded): like OCCT's fuzzy value but the kernel chooses and caps it per local feature size (smallest-edge/1.05 guardrail), so the caller never has to guess a global scalar.
4. **Partial-result mode** (degraded body plus fault report): always available as a fallback return when a perfect result is impossible.

### The escalation cascade (one operation, six rungs)

- **Tier 0:** filtered exact predicates (always on; a-robust-numerics).
- **Tier 1:** exact-construction strict boolean (clean input fast path).
- **Tier 2:** tolerant boolean with kernel-managed local tolerance, including in-loop local tolerance widening (Part 5) and sliver absorption.
- **Tier 3:** controlled perturbation (Halperin) for near-degeneracies needing a bounded, reported shape change; Simulation of Simplicity (Edelsbrunner-Mucke) for *exact* ties inside the exact tiers (no shape change).
- **Tier 4:** robust mesh boolean (Cherchi et al.), always-watertight output, tagged `fidelity: degraded`.
- **Tier 5:** SDF/volumetric wrap (Ansys-style), the absolute floor, tagged `fidelity: wrapped`.

Each rung records which tier produced the answer; the tier is itself a confidence grade.

### Graceful intake (the front door)

- Coherent orientation propagation per shell component, signed-volume inside/outside resolution, automatic normal correction, with every flipped face tagged (Part 6).
- Self-intersection detection with snap-resolution for near-coincident cases and flagging for genuine ones.
- Orientation repair runs after stitching and is re-validated to catch over-correction.

### The validate-after-degrade contract (the two hard rules)

- **Rule 1, geometry is negotiable:** best-effort operations may move geometry within a *reported* tolerance; every body answers `max_tolerance()` and per-entity `provenance()`, and carries a body-level epsilon-solid certificate (Qi-Shapiro) = max local tolerance.
- **Rule 2, topology is non-negotiable:** Keel never returns a body with invalid topology. Validity (orientability, closed loops, consistent incidence, watertightness for solids) is a hard postcondition enforced by an incremental post-operation check over touched entities. Any operation that cannot achieve valid topology escalates a tier or drops to the floor, never returns invalid topology. Geometric faults are reported and allowed; topological faults are blocked.

### Confidence reporting (honesty as a feature)

- Every operation returns `(Body, OpReport)`; `OpReport` carries status (clean / salvaged / degraded), the cascade tier reached, achieved tolerance, and a per-entity fault list with Parasolid-granular codes (open edge, bad vertex, bad order, reversed, self-intersecting loop, inconsistent orientation).
- Fault tags ride on the body's entities (not only the side report) so they survive serialization and downstream operations.
- Best-effort never silently succeeds on a salvaged body: residual faults are always reported; refinement is opt-in (CADfix doctrine).

### The non-negotiable invariants (true even on a degraded result)

1. **No panic, ever** (total function over all inputs).
2. **Valid topology, always** (orientable, closed, consistent incidence; watertight for solids).
3. **Bounded, reported geometric deviation** (achieved tolerance is known and queryable; the body is a certified epsilon-solid).
4. **Consistent classification** (a near-coincidence is never resolved two different ways within one operation).
5. **No silent salvage** (degradation is always tagged and reported).

### The robustness engine (the moat Keel can build)

- A regression corpus from day one, seeded from open STEP/IGES repositories and every OCCT-documented limitation case, growing with every dirty model that ever breaks Keel. Pass criterion per fixture: no panic, valid topology out, reported tolerance within budget.
- Determinism (kernel/14) so failures reproduce; fault-tagged results so failures self-describe; a never-panic fuzzing harness so the corpus runs unattended.
- Frictionless public contribution of failing models: the one robustness moat an open project can plausibly out-build a proprietary one, given users and discipline.

The synthesis in one sentence: **Keel should accept any input, escalate through a six-rung cascade that trades fidelity for success while bounding and reporting every trade, guarantee a valid-topology answer always (dropping to a mesh or SDF floor if it must), and never silently change a shape, because the difference between a kernel people trust on dirty data and one they do not is not cleverness but the discipline of salvageable geometry over inviolable topology, backed by a corpus that hardens against every failure it has ever seen.**

---

# References

**Vendor and product documentation**

1. Siemens Digital Industries Software. *Parasolid 3D Geometric Modeling.* https://plm.sw.siemens.com/en-US/plm-components/parasolid/ and http://www.parasolid.com/
2. Tech Soft 3D. *Parasolid: The Leading Modeling Kernel.* https://www.techsoft3d.com/products/parasolid
3. *Parasolid.* Grokipedia. https://grokipedia.com/page/Parasolid
4. Siemens. Parasolid Functional Description, ch. 9 *Checking*; `PK_BODY_check`; `PK_check_state_t`; `PK_FACE_check`. http://www.q-solid.com/Parasolid_Docs/chapters/fd_chap.09.html and http://www.q-solid.com/Parasolid_Docs/headers/pk_body_check.html
5. Spatial Corp. 3D ACIS Modeler, Kernel R10 Chapter 6, *Tolerant Modeling.* http://www-isl.ece.arizona.edu/ACIS-docs/PDF/KERN/06TMOD.PDF
6. Spatial Corp. *Healing in 3D interoperability: preserving design intent across CAD systems.* https://blog.spatial.com/healing-in-3d-interoperability
7. Spatial Corp. *Subtleties of B-rep Translation (Part 3): Why Healing Matters.* https://blog.spatial.com/3d-software-development-kits/subtleties-b-rep-translation-part-3-why-healing-matters
8. Open CASCADE Technology. *Boolean Operations* specification. https://dev.opencascade.org/doc/overview/html/specification__boolean_operations.html
9. Open CASCADE Technology. `BOPAlgo_Options` / `SetFuzzyValue`. https://dev.opencascade.org/doc/refman/html/class_b_o_p_algo___options.html
10. Open CASCADE Technology forums. *Boolean operations: in search for a robust process*; *Fuzzy Boolean Operations*; *how to guess the fuzzy value*; *opencascade and Boolean operations.* https://dev.opencascade.org/content/boolean-operations-search-robust-process
11. CADfix DX (ITI, a Wipro Company). *CAD Translation, Healing and Repair.* https://www.iti-global.com/interoperability-products/cadfix/cadfix-dx/ and https://www.cadinterop.com/en/your-needs/cad-repair-and-healing.html
12. C3D Labs. *C3D Modeler* (QA and 500,000-model regression corpus). https://c3dlabs.com/products/c3d-toolkit/modeler/
13. Ansys. *New Fluent Meshing Workflow Quickly Wraps Non-Watertight, Dirty Geometries.* https://www.ansys.com/blog/fluent-meshing-workflow-wraps-non-watertight-geometries

**Academic literature**

14. Jackson, D. J. (1995). Boundary representation modelling with local tolerances. *SMA '95*, 247-253. https://doi.org/10.1145/218013.218067
15. Fang, S., Bruderlin, B., & Zhu, X. (1993). Robustness in solid modelling: a tolerance-based intuitionistic approach. *Computer-Aided Design, 25*(9), 567-576. https://doi.org/10.1016/0010-4485(93)90072-V
16. Qi, J., & Shapiro, V. (2006). epsilon-topological formulation of tolerant solid modeling. *Computer-Aided Design, 38*(4), 367-377. https://doi.org/10.1016/j.cad.2005.10.010
17. Qi, J., & Shapiro, V. (2006). Geometric Interoperability With Epsilon Solidity. *J. Comput. Inf. Sci. Eng., 6*(3), 213-220.
18. Edelsbrunner, H., & Mucke, E. P. (1990). Simulation of Simplicity: a technique to cope with degenerate cases in geometric algorithms. *ACM TOG, 9*(1), 66-104. https://doi.org/10.1145/77635.77639 (arXiv math/9410209)
19. Halperin, D., & Shelton, C. R. (1998). A perturbation scheme for spherical arrangements with application to molecular modeling. *Computational Geometry, 10*(4).
20. Mehlhorn, K., et al. *Reliable and Efficient Computational Geometry via Controlled Perturbation.* ICALP 2006. https://people.mpi-inf.mpg.de/~mehlhorn/ftp/ControlledPerturbationGeneralStrategy.pdf
21. Sharma, V., & Yap, C. K. *Robust Geometric Computation*, Handbook of Discrete and Computational Geometry, ch. 45. https://www.csun.edu/~ctoth/Handbook/chap45.pdf
22. Hoffmann, C. M. (1989). The Problems of Accuracy and Robustness in Geometric Computation. *IEEE Computer, 22*(3), 31-41. https://www.cs.purdue.edu/cgvlab/www/resources/papers/Hoffmann-IEEE-1989-The_problems_of_accuracy_and_robustness_in_geo_computation.pdf
23. Cherchi, G., Pellacini, F., Attene, M., & Livesu, M. (2022). Interactive and Robust Mesh Booleans. *ACM TOG (SIGGRAPH Asia), 41*(6). https://doi.org/10.1145/3550454.3555460 (arXiv:2205.14151)
24. CGAL. *3D Boolean Operations on Nef Polyhedra* user manual. https://doc.cgal.org/latest/Nef_3/index.html

**Practitioner accounts and tertiary**

25. Novedge. *Design Software History: Boolean Modeling in CAD: CSG Origins, B-Rep Breaking Points, and Robustness Solutions.* https://novedge.com/blogs/design-news/design-software-history-boolean-modeling-in-cad-csg-origins-b-rep-breaking-points-and-robustness-solutions
26. Friedl, S. (2016). *Cork: A High Performance Library for Geometric Boolean/CSG Operations.* https://stephanfr.com/2016/03/21/cork-a-high-performance-library-for-geometric-booleancsg-operations/
27. Hacker News threads on Parasolid/ACIS vs OpenCASCADE robustness. https://news.ycombinator.com/item?id=31474409 ; https://news.ycombinator.com/item?id=11536518 ; https://news.ycombinator.com/item?id=15392547
28. *Engineering.com.* *Shaping Up: Spatial's ACIS Geometric Kernel Catching Up to Parasolid?* https://www.engineering.com/shaping-up-spatials-acis-geometric-kernel-catching-up-to-parasolid/
29. MeshLab *Reorient faces coherently* / Rhino *UnifyMeshNormals* (coherent orientation propagation). https://novedge.com/blogs/design-news/rhino-3d-tip-unify-mesh-normals-to-fix-shading-and-orientation-issues
30. W. R. Franklin. *Implementing Simulation of Simplicity for Geometric Degeneracies* (2022). https://wrfranklin.org/p/234-implementing-sos-2022.pdf
