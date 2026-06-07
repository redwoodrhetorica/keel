# Parametric Regeneration Architecture

Research dossier for the Keel B-rep kernel (Rust, Parasolid-class ambition).

## Scope and why this matters for drawing the kernel boundary

A history-based CAD application stores a model as a program: an ordered (and partially ordered) list of features ("sketch this profile", "pad it 20mm", "fillet that edge", "pattern the result"). On every parameter edit the program is re-evaluated and a fresh B-rep is produced. The machinery that holds that program, decides what to recompute, evaluates features in dependency order, caches intermediate bodies, parallelizes independent branches, and copes with features that fail to regenerate is the **parametric regeneration architecture**. It sits *above* the geometry kernel. The kernel offers modeling operators (extrude, boolean, blend, shell) plus the lineage reporting researched in the persistent-naming dossier (file 07); the regeneration layer is the consumer of those operators and that lineage.

This file researches that consumer side for two reasons. First, to confirm what stays **out** of Keel's scope: the feature tree, the dependency DAG, the rollback bar, branch/merge of histories, and the feature-failure UX are application concerns, not kernel concerns. A kernel that tries to own the feature model becomes a monolithic CAD system and loses the "embeddable kernel" property that makes Parasolid valuable. Second, to extract the **hard obligations the regeneration layer pushes back onto the kernel**: deterministic re-evaluation, cheap state snapshot/restore for rollback, stable entity identity across runs (lineage), the ability to roll back to and resume from an intermediate body, and a failure model that lets a single operation fail without corrupting the session. These are kernel-shaped requirements that must be designed in before the topology and operator layers freeze.

The literature splits cleanly into: (1) the foundational formalisms of feature-based, history-based modeling (Shah & Mantyla, Hoffmann's EREP); (2) dependency-graph evaluation semantics (FreeCAD's DAG, rollback bars, OnShape microversions); (3) minimal/incremental recompute; (4) parallel and speculative regeneration (thin, and that thinness is itself a finding); (5) the "parametric fragility" literature quantifying how often shared models break on edit, plus the methodology responses (resilient modeling, horizontal modeling, centrality analysis); (6) alternative paradigms as contrast (direct modeling, hybrid, code-CAD, node-graph dataflow); and (7) collaborative/cloud regeneration including CRDT-for-CAD. This dossier covers roughly 22 sources. Each entry gives Citation, Content, Limitations, and Kernel relevance, and closes with a synthesis of what regeneration demands from Keel and what stays above the line.

---

# PART 1: History-based modeling formalisms

## 1.1 The foundational texts

### Shah & Mantyla (1995): Parametric and Feature-Based CAD/CAM

**Citation.** Shah, J. J., & Mantyla, M. (1995). *Parametric and Feature-Based CAD/CAM: Concepts, Techniques, and Applications*. New York: John Wiley & Sons. 619 pp. ISBN 0-471-00214-3.

**Content.** This is the canonical academic treatment that codified feature-based design just as parametrics moved from labs into commercial CAD (Pro/ENGINEER had shipped in 1988). The book defines a **feature** as a generic shape with engineering meaning, and distinguishes **feature creation paradigms**: interactive feature recognition (extract features from a finished B-rep), design-by-features (build the model directly out of feature instances), and the parametric/history approach. It formalizes the feature model as carrying both **geometry** and **semantics** (parameters, constraints, attributes), and it lays out the parent-child dependency structure that underpins history replay: a feature's definition references entities and parameters of earlier features, so editing an early feature forces re-evaluation of its descendants. It treats parameterization, constraint-based dimensioning, and the difference between **procedural** (a sequence of construction operations) and **declarative** (a set of constraints to satisfy) model definitions. It also documents feature taxonomies, feature mapping between application views (design vs manufacturing features), and validity/feature-interaction problems (a later feature invalidating an earlier feature's intent).

**Limitations.** Predates cloud CAD, direct/hybrid modeling, and the persistent-naming literature's maturation; the dependency-graph and regeneration-scheduling machinery is described conceptually rather than as concrete algorithms. Manufacturing-feature emphasis is now partly dated.

**Kernel relevance.** Establishes the vocabulary the rest of this file uses (feature, parent-child dependency, procedural vs declarative, feature interaction) and confirms the architectural split: the feature model with its semantics lives above the kernel; the kernel realizes each feature as geometry. Keel should expose modeling operators rich enough to be the realization layer of any feature ("the feature is the recipe, the operator is the step") without itself defining what a feature is.

### Hoffmann & Juan (1992): EREP, the editable representation

**Citation.** Hoffmann, C. M., & Juan, R. (1992). *EREP: An Editable, High-Level Representation for Geometric Design and Analysis* (Technical Report CSD-TR-92-055 / CER-92-24). Purdue University, Department of Computer Sciences. See also the EREP project overview in Wozny, M. J., et al. (Eds.), *Geometric Modeling for Product Realization* (IFIP), Springer.

**Content.** EREP proposed a **high-level, generative, textual, modeler-independent** representation for feature-based design, separate from any specific solid modeler's B-rep. The EREP "program" is the *editable* artifact: it records the design as features, datums, constraints, and references, and an EREP processor re-evaluates it against an underlying kernel to produce geometry. Crucially, EREP anticipated the central problems of regeneration architecture: it required **persistent references** to model entities that survive re-evaluation (the seed of the persistent-naming problem), it separated the **design intent** (the EREP) from its **geometric instantiation** (the B-rep), and it argued the high-level representation should be neutral so a design could be archived and re-evaluated on different kernels. Datums (reference planes/axes) give stable, parameter-independent anchors to attach features to, reducing reliance on regenerated geometry.

**Limitations.** A representation proposal, not a shipping regeneration engine; the hard parts (robust name matching, incremental recompute, failure recovery) were named but not fully solved. Neutrality across kernels was never realized in practice because operator semantics differ between kernels.

**Kernel relevance.** EREP is the conceptual blueprint for the entire history/regeneration stack that sits above Keel. Its lesson for the kernel boundary is sharp: the durable, editable model is a high-level program held by the application, and the kernel's job is faithful, deterministic re-evaluation plus stable references. The "datum as stable anchor" idea argues Keel should support construction geometry (datum planes/axes/points) as first-class persistent entities the application can reference instead of regenerated faces.

### "A note on solid modeling: history, state of the art, future" (2023)

**Citation.** (Survey, arXiv:2302.14373, 2023.) A note on solid modeling: history, state of the art, future.

**Content.** A modern retrospective situating history-based parametric modeling within the broader arc of solid modeling (CSG, B-rep, feature-based, and the recent neural/implicit turn). It restates the replay model (model = ordered operations, re-evaluated on edit), names persistent naming as the chronic weakness of history-based systems, and contrasts history-based CAD with direct (history-free) modeling and with emerging representation-learning approaches. Useful as an up-to-date framing of where procedural parametric modeling sits relative to declarative and learned alternatives.

**Limitations.** A survey: breadth over depth, no new algorithms, light on regeneration-engine internals.

**Kernel relevance.** Confirms that history-based replay remains the dominant paradigm Keel must serve, while flagging that a kernel designed only around a feature-replay client may underserve direct-modeling and code-CAD clients. Argues for a kernel API that is paradigm-agnostic: operators plus lineage, with no assumption that a feature tree exists.

---

# PART 2: Dependency graphs and DAG evaluation semantics

## 2.1 The feature dependency DAG

A history-based model is not a flat list but a **directed acyclic graph** of features. Edges are dependencies: feature B depends on feature A if B's definition references A's parameters or A's geometry. Two dependency kinds recur in the literature and matter for the kernel boundary:

- **Explicit references**: B was authored to point at A (B's sketch is on A's face; B's pattern references A's body). These are recorded by the application at authoring time.
- **Geometric/implicit dependencies**: B happens to interact with A's geometry even though the user never declared it (a fillet that bridges two faces created by different earlier features). These are discovered only by evaluating geometry, which is where kernel lineage reporting becomes load-bearing.

Evaluation semantics: compute a **topological sort** of the DAG, then evaluate features in that order, each feeding the resulting body forward. A **rollback bar** (Pro/ENGINEER's "insert here") is a cursor in the linear (sorted) history that suppresses everything after it, so the user edits as if downstream features did not exist. Multiple timelines/branching histories generalize the single linear order to a tree of model states (see OnShape, Part 7).

### FreeCAD: the document dependency graph and recompute

**Citation.** FreeCAD project. *App::Document* and *App::DocumentObject* class references; FreeCAD documentation wiki, "Topological naming problem" and "Topological Naming Project". (freecad.github.io SourceDoc; github.com/FreeCAD/FreeCAD-documentation.)

**Content.** FreeCAD is the most fully documented open-source regeneration engine. A document is a **DAG of DocumentObjects** stored as an adjacency list, which (per the docs) "gives the opportunity to calculate the shortest recompute path and enables more complicated dependencies beyond trees." Each object keeps bidirectional links: **InList** (objects that link *to* it) and **OutList** (objects it links to). Recompute is driven by a **touched** flag: marking an object touched means everything in its InList (transitive dependents) must recompute, computed via `recompute()` which returns the count of recalculated features. Key graph operations are first-class: `topologicalSort()`, `getRootObjects()`, `checkOnCycle()` (the graph must be acyclic), `getDependencyList()` with options `DepSort` (topological), `DepNoXLinked`, `DepNoCycle`. Failure handling is explicit: `getErrorDescription()` per object, plus status flags `IgnoreErrorOnRecompute` and `RestoreError`, so a single object's error is recorded and propagated rather than aborting the whole document. FreeCAD's well-known weakness is the persistent-naming/topological-naming problem at the leaf level (links to specific faces/edges break on recompute), which the project has been actively re-engineering.

**Limitations.** Open-source maturity and historical TNP fragility; recompute is single-threaded in practice; the adjacency-list DAG is application-level and tightly coupled to OCCT's (separate) kernel. Not a performance reference.

**Kernel relevance.** The cleanest concrete template for the application-side DAG that will sit above Keel, and a precise demonstration of the boundary: the DAG, touched-propagation, topological sort, and cycle checking are all *application* logic over kernel-produced shapes. What FreeCAD needs *from* the kernel is exactly two things Keel must provide: deterministic operators and stable leaf-entity identity (lineage) so InList/OutList links to faces and edges do not silently rebind. FreeCAD's `IgnoreErrorOnRecompute` pattern argues Keel operators must return structured errors (not panics) so the host can record per-feature failure and continue.

---

# PART 3: Minimal and incremental recompute

### Dependency-pruned regeneration (locally updating a 3D model)

**Citation.** Locally updating a three-dimensional model (US Patent 8,305,376, assigned to Dassault Systemes SolidWorks). Related: Model management technology using grouping of features (US Patent 7,313,504).

**Content.** The core minimal-recompute idea, as stated in the SolidWorks patent: "determine the extent of a model that requires regeneration when a modification is made by analyzing parent-child, topological, and geometric relationships between CAD entities, and only those CAD entities that may affect the model differently are regenerated." The result is that "update time is limited to and dependent on the size and complexity of a subset of model features rather than the size and complexity of the entire model." This is dependency-pruned regeneration: starting from the edited feature, walk the dependents (InList closure) and recompute only that affected sub-DAG, leaving unaffected features' geometry untouched. The grouping/scope patent adds **feature scope**: a feature's design effect can be limited to a feature chain so that operations in one chain do not affect features in another chain, making feature creation order and regeneration order more independent and enlarging the set of features that need not recompute.

**Limitations.** Patents, so framed as claims rather than published algorithms with benchmarks; "geometric relationships" (implicit dependencies) are the hard part and the patents gloss the detection mechanism. Correctness of pruning depends entirely on the dependency graph being complete, including implicit geometric dependencies the user never declared.

**Kernel relevance.** Pruning is an application-layer decision (which features to skip) but its *correctness* depends on the kernel. If a pruned feature is wrongly assumed independent because an implicit geometric dependency was missed, the model is silently wrong. Therefore Keel must give the host enough lineage and interference information to detect implicit dependencies (e.g., "this blend consumed faces from bodies A and B"), and operators must be deterministic so that "we did not recompute it, so it is unchanged" is actually true. This is the strongest single argument that lineage reporting is a kernel obligation, not an optional extra.

### OnShape regeneration: deterministic, cached, current-state

**Citation.** Onshape (PTC). Engineering material and help: Performance Panel documentation; forum/blog discussion of Part Studio regeneration and caching; "How Onshape Has Fundamentally Improved Parametric CAD".

**Content.** OnShape's published regeneration model has three notable properties. (1) **Current-state by default**: unlike older modelers, a Part Studio "stays in a current regen state"; you only pay regeneration cost when you actually change a feature or roll the feature bar back, at which point it rebuilds. (2) **Caching keyed on immutable definitions**: because each model state is an immutable microversion (Part 7), regeneration results can be cached and reused; "if the cache is valid, then there is no regen of a Part Studio." Referencing a *version* (immutable) rather than a live *workspace* in a derived feature lets the derive reuse cached geometry instead of rebuilding the parent. (3) **Server-side, deterministic regeneration**: regeneration runs on OnShape's servers and is deterministic, which is what makes cross-client caching and collaborative consistency possible. The Performance Panel exposes per-feature regen timings so users can find expensive features (large patterns, heavy fillets) that dominate recompute.

**Limitations.** Published mostly as marketing/help material, not peer-reviewed internals; exact cache-invalidation granularity (whole Part Studio vs sub-feature) is only partially documented. Determinism guarantees are asserted, not formally specified publicly.

**Kernel relevance.** OnShape is the proof that **determinism plus immutable definitions equals cacheability**, and caching is the dominant regeneration-performance lever. For Keel this means: (a) operators must be deterministic given identical inputs (no time/address/iteration-order nondeterminism); (b) the kernel must support cheap, reliable **snapshot of an intermediate body** so the host can cache "the body after feature N" and resume from it; (c) bodies need stable content identity so a cache key can be formed. These are kernel features; the cache policy itself stays in the host.

---

# PART 4: Parallel and speculative regeneration

### The state of parallel feature-history evaluation (a thinness finding)

**Citation.** (Synthesis of the available literature; representative pointers: speculative-execution theory in Hank et al., *Speculative Execution and Branch Prediction on Parallel Machines*, ICS 1993; OnShape per-branch independence; SolidWorks multi-body/feature-scope patents.)

**Content.** This is a deliberate negative finding: **published, peer-reviewed work on parallelizing the evaluation of a feature history is thin.** The general parallel/speculative-execution literature is mature (branch prediction, speculative parallel evaluation of trees) but is about CPU instruction streams and classification trees, not CAD feature DAGs. In CAD practice, parallelism appears in three indirect forms rather than as a published "parallel regen" algorithm: (1) **independent-branch parallelism**, where two features with no dependency path between them (e.g., features in two separate bodies, or two leaves of the DAG) could in principle be evaluated concurrently, enabled structurally by **multi-body** modeling and the **feature-scope** idea from the SolidWorks patents that isolates a feature's effect to one chain; (2) **speculative evaluation**, evaluating likely-next branches before a merge resolves, which OnShape's architecture (both branches can be prepared) gestures at for branch/merge but does not document as a feature-level regen optimization; (3) **operator-internal parallelism** (a single fillet or tessellation parallelized inside the kernel), which is a kernel concern, not a history-scheduling concern. The dominant industrial regeneration engines are effectively single-threaded at the feature-scheduling level; performance comes from caching and pruning, not from parallel feature evaluation.

**Limitations.** Absence of evidence is not evidence of absence: vendors may parallelize internally without publishing. The finding is about the *open literature*, which is genuinely sparse on feature-DAG parallelism.

**Kernel relevance.** Two implications. First, the regeneration scheduler (which decides what runs concurrently) stays above the kernel. Second, for the host to *ever* parallelize independent branches, the kernel must support **concurrent evaluation against disjoint bodies safely**: thread-safe operators, no hidden global mutable state, and bodies that can be built independently and merged later. Rust's ownership model is a strong fit here and a differentiator. Keel should target "operators are pure functions over bodies, safe to run on disjoint inputs concurrently," which makes host-level branch parallelism *possible* without the kernel itself owning a scheduler.

---

# PART 5: Parametric fragility and modeling-methodology responses

This is the empirical literature on how badly history-based models break on edit, and the prescriptive methodologies that try to fix it by discipline rather than by kernel technology. It is the strongest evidence that fragility is largely an *authoring/architecture* problem above the kernel, while also pinpointing the residual kernel contribution.

### Camba, Contero & Company (2016): strategies for design reusability

**Citation.** Camba, J. D., Contero, M., & Company, P. (2016). Parametric CAD modeling: An analysis of strategies for design reusability. *Computer-Aided Design, 74*, 18-31. https://doi.org/10.1016/j.cad.2016.01.003

**Content.** A systematic study of three published **formal modeling methodologies** for history-based parametric design: **Delphi/explicit-reference modeling**, **horizontal modeling**, and the **resilient modeling strategy (RMS)**. The paper frames the problem precisely: because feature-based parametric modeling rests on **parent-child interdependencies**, the same part can be built many ways, and the chosen way determines the model's flexibility (ease of editing) and reusability (ease of repurposing). It tests the methodologies on parametric models of varying complexity and finds that formal methodologies measurably improve robustness and reusability over ad hoc modeling, chiefly by **reducing and rationalizing parent-child dependencies** so that an edit's blast radius is smaller and more predictable. It connects model quality to dimensioning/constraint schemes and design-intent communication.

**Limitations.** Studies relatively small model sets and expert-authored models; "robustness" is partly assessed via expert judgment rather than a large-sample failure-rate measurement. Methodology benefits depend on disciplined humans.

**Kernel relevance.** The headline lesson for Keel: **most fragility is created by the dependency structure the application/author builds, not by the kernel.** The kernel cannot make a badly structured feature tree robust. What the kernel *can* do is make the dependency structure *legible* (accurate lineage so the host and methodologies can see and minimize parent-child links) and make references stable enough that disciplined references actually hold. This validates keeping the feature tree and methodology above the kernel while sharpening Keel's lineage obligation.

### Gebhard: the Resilient Modeling Strategy (RMS)

**Citation.** Gebhard, R. *The Resilient Modeling Strategy* (CAD-neutral methodology; learnrms.com / resilientmodeling.com). Presented widely, e.g., Siemens PLM community "A Resilient Modeling Strategy" (SEU13-122). Popularized in the Engineering.com series "The failed promise of parametric CAD."

**Content.** RMS is a practitioner methodology, not a system: a prescribed way to organize the feature tree so models do not "blow up" on edit. It partitions the tree into **named, ordered groups** (commonly: Reference, Construction, Core/main shape, Detail, Modify, Quarantine) and enforces ordering and reference rules so that fragile, geometry-dependent features (fillets, drafts, shells) live late and reference stable upstream construction/datum geometry rather than each other. The explicit aim is to **limit parent-child relationships** and make the tree legible so any engineer can edit a stranger's model without cascading failures. The accompanying critique ("the failed promise of parametric CAD") argues parametric fragility is so endemic that disciplined structure is the only practical remedy short of changing the paradigm.

**Limitations.** Pure discipline: it relies on humans following rules, offers no enforcement, and does not eliminate the underlying naming/dependency fragility, it only routes around it. CAD-neutral by design, so it provides no kernel guidance directly.

**Kernel relevance.** RMS is a workaround for two kernel-adjacent weaknesses: unstable references and uncontrolled dependency blast radius. Its existence is evidence that (a) the kernel should make **datum/construction geometry first-class and stable** (RMS leans heavily on referencing construction geometry over regenerated faces, echoing EREP datums), and (b) the kernel should report dependencies accurately so tools could one day *enforce* RMS-style structure automatically. The methodology itself stays entirely above Keel.

### Ma, Cheng & Du (2019): graph centrality of feature dependencies

**Citation.** Ma, Y., Cheng, Z., & Du, Y. (2019). Graph centrality analysis of feature dependencies in CAD models. *Computer-Aided Design and Applications, 16*(4), 684-694. (CAD&A vol. 15(5), 2018 preprint listing.)

**Content.** Treats the feature dependency graph as a **DAG** (features = nodes, dependencies = edges) and applies **graph-centrality metrics** (degree, betweenness, closeness) to quantify each feature's structural importance. High-centrality features are dependency **bottlenecks**: editing them triggers cascading recompute and edit failures, so models that concentrate dependencies in a few high-centrality features are brittle and hard to edit, whereas distributing dependencies across several moderately-central features improves flexibility. Validated on case parts (connecting rod, automotive seat); centrality is shown to **predict edit vulnerability**, giving an objective metric to guide model architecture.

**Limitations.** Analyzes only declared/explicit dependencies; implicit geometric dependencies (the ones only the kernel can reveal) are not captured by the authored graph, so centrality may underestimate true fragility. Metric-to-failure correlation is demonstrated, not causally proven.

**Kernel relevance.** Reinforces that fragility is a property of the dependency *graph*, an application artifact, and that minimizing/balancing it is an authoring concern above the kernel. But it also exposes a gap only the kernel can fill: a complete fragility analysis needs the **implicit geometric dependencies** that centrality-on-authored-edges misses, and those come from kernel lineage/interference reporting. Keel should make the *true* dependency graph (declared plus geometric) discoverable.

---

# PART 6: Alternative paradigms as contrast

### Direct modeling and hybrid (history + direct): NX/Solid Edge synchronous, Fusion 360

**Citation.** Industry/technical sources: Siemens, "Understanding parametric and direct modeling in modern CAD tools" (synchronous technology in NX and Solid Edge); Autodesk, "Direct Modeling and Parametric Modeling in Fusion 360"; Engineering.com, "What's the difference between parametric and direct modeling?".

**Content.** **Direct modeling** edits the B-rep in place (push/pull a face, move a hole) with **no feature history recorded**, so there is no regeneration and no parent-child fragility, at the cost of losing parametric design intent. **Hybrid** systems combine the two. Siemens **synchronous technology** (NX, Solid Edge) infers geometric conditions (coplanarity, symmetry, concentricity) on the fly and applies direct edits while honoring inferred relations, blending direct edits with live constraints inside (or alongside) a history. **Fusion 360** offers parametric (design history captured) or direct (history off) modes, but switches the whole document between them rather than mixing per-edit. The common thread: direct/synchronous edits operate on the *current* B-rep and need robust **local operations** (file 03) and recognition of geometric conditions, not history replay.

**Limitations.** Direct modeling sacrifices editable design intent; hybrid systems carry the complexity of both worlds and still must reconcile a direct edit with subsequent history replay (a hard consistency problem). Synchronous-technology internals are proprietary.

**Kernel relevance.** Critical scoping evidence: a good kernel must serve **both** a history-replay client and a direct/hybrid client. That means Keel must provide first-class **local/direct operations** (move face, offset, delete-face-and-heal) and **geometric-condition queries** (are these faces coplanar/concentric?) as kernel primitives, independent of any feature history. The history layer is optional client policy; the kernel must not assume it exists. This argues for a kernel API centered on body-state transforms plus interrogation, with history strictly above.

### Code-as-CAD: OpenSCAD CSG-tree re-evaluation; CadQuery / build123d on OCCT

**Citation.** OpenSCAD project documentation (CSG tree / AST evaluation); CadQuery project (github.com/CadQuery/cadquery), "a Python parametric CAD scripting framework based on OCCT"; build123d documentation; arXiv:2408.01801, *Introducing Bidirectional Programming in Constructive Solid Geometry-Based CAD* (2024); arXiv:2408.01815, *Facilitating the Parametric Definition of Geometric Properties in Programming-Based CAD* (2024).

**Content.** In **code-CAD** the model *is* a program in a general-purpose or DSL language; regeneration is simply **re-running the script**. **OpenSCAD** parses source to an AST, evaluates language constructs and variables down to numbers, and produces an **abstract CSG tree** that is then meshed (historically via CGAL); the whole tree is re-evaluated on any change, and persistent naming is sidestepped because the user references *code variables and constructed objects*, not regenerated B-rep faces. **CadQuery** and its successor **build123d** are Python frameworks over **OCCT** (a full B-rep kernel), giving NURBS, fillets, STEP I/O, and feature-stack-style modeling from code; build123d notably supports both CSG-tree and feature-stack workflows. The bidirectional-programming paper studies the inverse problem (edit geometry, propagate edits back to source), which is the code-CAD analogue of direct editing.

**Limitations.** OpenSCAD's full-tree re-evaluation is simple but does no incremental recompute, so large models are slow; CGAL mesh-CSG lacks NURBS/B-rep richness. Code-CAD references entities positionally/programmatically, which avoids B-rep naming but introduces its own fragility (selectors that match the wrong face after a change). Bidirectional editing is early-stage research.

**Kernel relevance.** Code-CAD is a major and growing client class for an embeddable Rust kernel (a Rust code-CAD ecosystem is a natural Keel target). Its requirement is minimal and clean: **deterministic operators callable from a host program, returning bodies the host holds and re-runs**. There is no feature tree to manage; the "history" is the user's source code. This validates a kernel API that is a library of pure-ish operators plus selectors/queries (to pick faces/edges programmatically by geometric criteria), and it argues Keel must offer robust **geometric selectors** (by position, by adjacency, by tag) since code-CAD has no GUI picking. The CSG/NURBS contrast (CGAL vs OCCT) argues Keel should be B-rep/NURBS-first so it can serve the high end of code-CAD that OpenSCAD cannot.

### Node-graph / dataflow CAD: Grasshopper evaluation semantics

**Citation.** Robert McNeel & Associates, Grasshopper (for Rhino). Developer guides: "Advanced Data Structures" (data trees), Rhino developer documentation; community treatments of Grasshopper data trees and evaluation.

**Content.** Grasshopper is a **visual dataflow** language: components (nodes) with input/output ports wired into a **directed graph**; data flows along wires as **data trees** (hierarchical lists). Evaluation is dependency-driven: when an upstream parameter changes, downstream components **recompute in dependency order**, and unchanged subgraphs need not recompute (a dataflow analogue of dependency-pruned regeneration). Data-tree structure controls how components **match inputs** (longest-list, cross-reference, graft) and thus how iteration and broadcasting happen. The dataflow model makes parallel/independent-branch evaluation natural in principle (disjoint subgraphs are independent) and makes caching of component outputs a standard optimization.

**Limitations.** Dataflow graphs explode in size and become hard to read ("spaghetti"); data-tree matching semantics are a notorious source of subtle bugs. Grasshopper's geometry is Rhino's (NURBS surfaces, lighter on solid B-rep robustness than a Parasolid-class kernel). Evaluation is largely single-threaded per solution despite the latent parallelism.

**Kernel relevance.** A node-graph CAD client wants exactly the same kernel contract as code-CAD plus the regeneration layer's structure: **deterministic operators, output caching keyed on inputs, dependency-ordered re-evaluation of only the changed subgraph.** The dependency graph and the data-tree matching are host concerns; the kernel supplies operators and queries. The latent (rarely exploited) parallelism in dataflow reinforces the Part 4 finding and the recommendation that Keel operators be safe to run concurrently on disjoint inputs.

---

# PART 7: Cloud, collaborative, and branching regeneration

### OnShape microversions, versions, branches, and merge

**Citation.** Onshape (PTC). "Under the Hood: How Collaboration Works" (engineering blog); help documentation "Working with Versions, Branching, and Merging" and "Merging".

**Content.** OnShape's data model is the most relevant industrial example of regeneration-as-version-control. Each Part Studio state is stored as an **immutable microversion**: every edit (change an extrude depth, rename a part, drag a sketch) creates a new microversion holding a **reference to its parent microversion plus the change itself**. Microversions form a **tree**; a **workspace** (branch) tracks its latest microversion. Crucially, a stored change is expressed against **internal feature IDs** ("change the depth of Extrude 1 to 4 in") so it is **rebasable**: the change "is intended to apply to the parent microversion, but could be applied to a different one," which is what makes branching, merging, and collaborative undo work. **Merge** finds changes present in the source branch but not the target and creates a microversion applying them; conflicts are detected **at the feature level** before geometry is merged (conflicting features are flagged and not auto-changed). **Collaborative undo** applies the *inverse* of a user's change on top of the latest microversion (rather than reverting to an old state), preserving collaborators' work. Regeneration is server-side and deterministic, so any client reconstructs identical geometry from a microversion.

**Limitations.** Proprietary; the blog is high-level. Feature-level conflict detection prevents *geometry* corruption but cannot detect all *semantic* design-intent conflicts. Rebasability depends on stable feature IDs, which is itself a persistent-identity problem.

**Kernel relevance.** Branch/merge/microversion machinery is squarely an **application** concern, but it imposes two non-negotiable kernel requirements. (1) **Deterministic regeneration**: identical definition implies identical geometry on any machine, or caching and collaboration both break. (2) **Stable internal entity IDs** that survive replay, so changes expressed against feature/entity IDs remain meaningful when rebased onto a different parent, this is the persistent-naming obligation again, now driven by version control rather than just edit-robustness. Keel must guarantee bit-stable, ID-stable re-evaluation; the microversion tree, diff, and merge all stay above.

### CRDT-based collaborative parametric CAD

**Citation.** Wu, Q., et al. (2018). A novel CRDT-based synchronization method for real-time collaborative CAD systems. *Computer-Aided Design and Applications* / *Future Generation Computer Systems* line of work. Related: "CRDT-based Conflict Detection and Resolution for Massive-scale Real-time Collaborative CAD systems" (CCSCW 2017); "Integrating selective undo of feature-based modeling operations for real-time collaborative CAD" (*FGCS*, 2019); "A Paradigm for Collaborative 3D Editing via List CRDTs" (CSAE 2023, ACM 10.1145/3627915.3627919).

**Content.** This research line adapts **conflict-free replicated data types** from collaborative text editing to feature-based CAD. The model is represented as an **ordered list (or graph) of modeling operations**; a **list CRDT** gives each operation a stable position so concurrent inserts/edits from multiple users converge to the same sequence under **strong eventual consistency** without a central lock. The work adds CAD-specific **conflict detection** that understands feature **dependency relations** (so a concurrent edit that deletes a parent referenced by another user's new child is recognized and resolved), and **selective undo** of individual feature operations in a collaborative setting. The aim is real-time multi-user parametric editing where every replica regenerates consistent geometry.

**Limitations.** Mostly academic prototypes, not production kernels; CRDTs guarantee convergence of the *operation list*, not that the resulting *geometry* is what users intended (a converged-but-wrong model is possible). Dependency-aware conflict detection is the hard part and remains incomplete for rich geometric interactions. OnShape's centralized-server microversion approach is the deployed alternative and arguably simpler.

**Kernel relevance.** Confirms there *are* published CRDT-for-CAD attempts (a direct answer to the scope question) but that they live entirely **above** the kernel: a CRDT orders operations, the kernel still just regenerates geometry from the resulting sequence. The kernel requirement is identical to OnShape's: **deterministic, ID-stable regeneration** so that every replica, regenerating the converged operation list, gets byte-identical geometry. Keel needs no CRDT awareness; it must only guarantee that "same operation sequence implies same body" so any convergence scheme above it yields convergent geometry.

---

# PART 8: Feature libraries, user-defined features, and multi-representation consistency

### User-defined features (UDFs)

**Citation.** Hoffmann, C. M., & Joan-Arinyo, R. (1998). On user-defined features. *Computer-Aided Design, 30*(5), 321-332. https://doi.org/10.1016/S0010-4485(97)00048-1. Industrial instances: Creo Parametric UDFs; Siemens NX User Defined Features via Knowledge Fusion (`ug_udfs`).

**Content.** A UDF packages a sub-graph of features (with their parameters, internal relations, and a list of **external reference placeholders**) into a reusable, parameterized macro that can be **instantiated** onto a host model by supplying values and resolving the external references (e.g., the face/edge to place it on). Hoffmann & Joan-Arinyo give the formal account: a UDF is a parameterized feature schema with a clear separation between its **internal** definition and its **interface** (placement references and parameters), instantiated by binding the interface to the host. NX's Knowledge Fusion `ug_udfs` class takes a UDF name, optional library, and parameter/reference bindings and inserts the feature group into the receiving part; Creo UDFs likewise capture selected features plus dimensions, relations, and reference lists.

**Limitations.** UDF robustness inherits all the persistent-naming fragility of ordinary features at the **placement interface** (the external references break exactly as normal references do). The 1998 formalism predates cloud/programmatic instantiation.

**Kernel relevance.** UDF instantiation is composition of feature operations, an **application** capability, but it stresses the same kernel primitives harder: instantiating a UDF re-binds external references to host geometry, so it depends on **robust geometric selection and lineage** to attach the UDF's placeholder references to the right host faces/edges. Keel needs no UDF concept; it needs reference/selector machinery good enough that the host can repeatedly bind a feature group's interface to varied host geometry.

### Multi-representation consistency: design-analysis association and drawing regeneration

**Citation.** Peak, R. S., Fulton, R. E., Nishigaki, I., & Okamoto, N. (1998). Integrating engineering design and analysis using a multi-representation approach. *Engineering with Computers, 14*(2), 93-114 (the MRA / PBAM framework). Hoffmann, C. M., & Joan-Arinyo, R. (2000). Distributed maintenance of multiple product views. *Computer-Aided Design, 32*(7), 421-431. Related CAD/CAE idealization work (feature-based multi-resolution / multi-abstraction modeling).

**Content.** Real CAD models drive *several* downstream representations that must stay consistent when the master model regenerates: a **simplified CAE/analysis model** (idealized: defeatured, mid-surfaced, dimensionally reduced), **2D drawings**, and manufacturing views. The **multi-representation architecture (MRA)** of Peak et al. makes the design-analysis link explicit via **PM-ABB idealization linkages** so that when the design model changes, the analysis model is regenerated and kept associatively consistent, rather than re-idealized by hand. Hoffmann & Joan-Arinyo's **multiple-view maintenance** addresses keeping several feature views of one product mutually consistent under edits (a change in one view propagating correctly to others). Drawing regeneration is the most familiar case: a model edit must re-derive dimensioned views, section views, and HLR output (file 06) associatively.

**Limitations.** Idealization is hard to fully automate and associate; large parameter changes can invalidate the idealization scheme (a rib that disappears breaks its mid-surface). Multiple-view consistency is formally fragile when views impose conflicting constraints.

**Kernel relevance.** Building and maintaining the analysis/drawing representations is **above** the kernel, but every one of these consumers depends on the same two kernel services: **associativity through stable identity** (the drawing's dimension references face F; after regen F must still be findable, lineage again) and **defeaturing/simplification operators plus HLR/section interrogation** (files 03 and 06) that the host calls to *build* the simplified or drawn representation. Keel's job is to expose simplification operators and stable references; orchestrating multi-representation regeneration is the application's.

---

# What regeneration demands from Keel (and what stays above)

**What stays above the kernel (confirmed out of scope).** The feature tree and its semantics; the dependency DAG and its topological-sort/touched-propagation/cycle-checking; the rollback bar and the linear-vs-DAG ordering; minimal-recompute *policy* (which features to skip); the regeneration scheduler and any branch parallelism *decisions*; the cache *policy*; user-defined-feature definition and instantiation; modeling methodologies (RMS, horizontal, explicit-reference) and fragility metrics; version/branch/merge of histories and any CRDT or microversion machinery; multi-representation orchestration (CAE idealization, drawing regen). FreeCAD, OnShape, Grasshopper, and the CRDT papers all implement these *over* a kernel, never inside it. Keel must not absorb them or it stops being an embeddable kernel.

**What regeneration pushes back onto the kernel (in-scope obligations).**

1. **Determinism.** Identical inputs must produce byte-identical, ID-identical output every run, on every machine. This is the keystone: caching (OnShape), pruning correctness (SolidWorks patents), collaboration convergence (CRDT, microversions), and code-CAD reproducibility all collapse without it. No nondeterministic iteration order, address-dependent tie-breaking, or wall-clock/RNG influence in operators.

2. **Stable entity identity / lineage.** Every operator must report Created/Modified/Split/Merged/Deleted relations between input and output entities (file 07). This is what makes references survive replay, what lets the host detect **implicit geometric dependencies** that authored DAGs miss (Camba/Contero, Ma centrality), what makes UDF re-binding and drawing/CAE associativity work, and what makes rebasable changes (OnShape) meaningful.

3. **Cheap snapshot/restore of body state.** The rollback bar and intermediate-body caching require the kernel to checkpoint "the body after feature N" and resume from it cheaply (echoes the Parasolid partition/pmark and ACIS delta models from file 07). Without resumable intermediate state, every edit forces a full replay from feature 1.

4. **Structured, non-fatal operator failure.** An operator must return a typed error (not panic, not corrupt the session) so the host can mark one feature failed, keep the rest of the model, and present skip/suppress/repair UX (FreeCAD `IgnoreErrorOnRecompute`, OnShape Repair Manager). The kernel owns failure *atomicity* (a failed op leaves the body unchanged); the host owns failure *policy* (skip/suppress/substitute).

5. **First-class datums/construction geometry and robust geometric selectors.** EREP datums and RMS both lean on stable, parameter-independent anchors; code-CAD and node-graph clients need to pick faces/edges programmatically by geometric criteria with no GUI. Keel should make construction geometry persistent and offer selectors (by position, adjacency, tag, geometric condition) plus coplanar/concentric-type queries (also serving direct/synchronous clients).

6. **Concurrency-safe operators over disjoint bodies.** To let any host parallelize independent branches (Part 4's thin-but-real opportunity), operators must be free of hidden global mutable state and safe to run on disjoint inputs concurrently. Rust's ownership model makes this a natural Keel differentiator; the *scheduling* still belongs to the host.

7. **Direct/local operations as kernel primitives.** Because direct and hybrid modeling are first-class client paradigms (NX synchronous, Fusion, code-CAD bidirectional editing), the kernel must offer move-face, offset, delete-face-and-heal, and similar local ops (file 03) independent of any history. The kernel must never assume a feature history exists.

The throughline: the regeneration architecture is a **scheduler and bookkeeper over deterministic, lineage-reporting, snapshot-able, fail-soft kernel operations.** Keel should ship those operations and that reporting, and ship *nothing* of the scheduler. Done right, one kernel serves history-replay CAD, direct/hybrid CAD, code-CAD, node-graph CAD, and collaborative/cloud CAD with the same API surface.

---

# References

1. Shah, J. J., & Mantyla, M. (1995). *Parametric and Feature-Based CAD/CAM: Concepts, Techniques, and Applications*. John Wiley & Sons.
2. Hoffmann, C. M., & Juan, R. (1992). *EREP: An Editable, High-Level Representation for Geometric Design and Analysis* (Tech. Rep. CSD-TR-92-055). Purdue University.
3. A note on solid modeling: history, state of the art, future. (2023). arXiv:2302.14373.
4. FreeCAD project. *App::Document* / *App::DocumentObject* class references and "Topological naming problem" documentation. freecad.github.io/SourceDoc; github.com/FreeCAD/FreeCAD-documentation.
5. Locally updating a three-dimensional model. US Patent 8,305,376 (Dassault Systemes SolidWorks).
6. Model management technology using grouping of features. US Patent 7,313,504.
7. Onshape (PTC). Performance Panel documentation and "How Onshape Has Fundamentally Improved Parametric CAD".
8. Hank, R. E., et al. (1993). Speculative execution and branch prediction on parallel machines. *Proc. 7th Int. Conf. on Supercomputing (ICS)*, 77-86.
9. Camba, J. D., Contero, M., & Company, P. (2016). Parametric CAD modeling: An analysis of strategies for design reusability. *Computer-Aided Design, 74*, 18-31. doi:10.1016/j.cad.2016.01.003.
10. Gebhard, R. *The Resilient Modeling Strategy* (learnrms.com; resilientmodeling.com). Siemens PLM community SEU13-122.
11. Ma, Y., Cheng, Z., & Du, Y. (2019). Graph centrality analysis of feature dependencies in CAD models. *Computer-Aided Design and Applications, 16*(4), 684-694.
12. Siemens, "Understanding parametric and direct modeling in modern CAD tools"; Autodesk, "Direct Modeling and Parametric Modeling in Fusion 360"; Engineering.com, "What's the difference between parametric and direct modeling?".
13. OpenSCAD project documentation (CSG tree / AST evaluation).
14. CadQuery project. github.com/CadQuery/cadquery; build123d documentation.
15. Introducing Bidirectional Programming in Constructive Solid Geometry-Based CAD. (2024). arXiv:2408.01801.
16. Facilitating the Parametric Definition of Geometric Properties in Programming-Based CAD. (2024). arXiv:2408.01815.
17. Robert McNeel & Associates. Grasshopper developer guides, "Advanced Data Structures" (data trees). developer.rhino3d.com.
18. Onshape (PTC). "Under the Hood: How Collaboration Works" (microversions, branching, merging) and Versions/Branching/Merging help.
19. Onshape (PTC). "Tackling History-Based Errors in Parametric CAD" (Repair Manager).
20. Wu, Q., et al. (2018). A novel CRDT-based synchronization method for real-time collaborative CAD systems. *Computer-Aided Design and Applications*. Related: CRDT-based Conflict Detection and Resolution for Massive-scale Real-time Collaborative CAD systems (CCSCW 2017); Integrating selective undo of feature-based modeling operations for real-time collaborative CAD (*FGCS*, 2019); A Paradigm for Collaborative 3D Editing via List CRDTs (CSAE 2023, ACM 10.1145/3627915.3627919).
21. Hoffmann, C. M., & Joan-Arinyo, R. (1998). On user-defined features. *Computer-Aided Design, 30*(5), 321-332. doi:10.1016/S0010-4485(97)00048-1.
22. Peak, R. S., Fulton, R. E., Nishigaki, I., & Okamoto, N. (1998). Integrating engineering design and analysis using a multi-representation approach. *Engineering with Computers, 14*(2), 93-114.
23. Hoffmann, C. M., & Joan-Arinyo, R. (2000). Distributed maintenance of multiple product views. *Computer-Aided Design, 32*(7), 421-431.
