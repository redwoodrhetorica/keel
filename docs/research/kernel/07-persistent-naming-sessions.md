# Persistent Naming and Kernel Session Management

Research dossier for the Keel B-rep kernel (Rust, Parasolid-class ambition).

## Scope and why this matters before topology freeze

Persistent naming (the "topological naming problem", TNP) is the discipline of keeping a stable handle on a face, edge, or vertex across parametric re-evaluation, even when the boundary representation changes its topology between runs. A history-based CAD system records features as a program ("pad this sketch", "fillet that edge"); on every edit the program is replayed and a fresh B-rep is built. If a downstream feature refers to "Edge7" and the replay renumbers edges, the reference silently rebinds to the wrong entity or breaks. This is the single most consequential adjacent topic to the topology layer, because the fix is structural: the kernel must report, per operation, which input entities each output entity descends from (the Modified / Generated / Deleted relation). That reporting cannot be bolted on after the topology data structures and Boolean/local-op engines are frozen, so it has to be designed in now.

Session management (undo, rollback, journaling, transactionality, concurrency) has thin academic literature but a very studyable industrial record: Parasolid's partition/pmark/delta model and ACIS's bulletin-board/delta-state model are both publicly documented. The two topics are linked: both demand that the kernel attach stable identity to entities and record what changed between two states.

This file collects roughly 22 sources. Per-source entries give Citation, Mechanism (in concrete detail), Failure modes, and Kernel relevance. It closes with the concrete hooks Keel must expose from day one.

---

# PART 1: Persistent naming

## 1.1 The problem statement

A parametric feature stores references to entities of a *previous* evaluation of the model. On re-evaluation the model is rebuilt from scratch, producing a new B-rep whose entities are, a priori, anonymous. The persistent-naming problem decomposes into two sub-problems, named consistently across the literature:

1. **Naming**: at authoring time, assign to each referenced entity a name that captures enough about how it came to exist that it can be recognized again.
2. **Name matching**: at re-evaluation time, given the stored name and the new B-rep (plus the new operation histories), find the entity in the new model that the name designates, or report that it no longer exists / has split / has merged.

The hard cases are all topology-change cases: an entity **splits** (one face becomes two after an intersecting feature), entities **merge** (two faces become one when a separating feature is removed), an entity **disappears**, or a new entity **appears**. References to edges are the worst, because a B-rep edge is the intersection of two faces and is therefore doubly sensitive to parameter change: a small dimensional tweak can change which faces intersect and thus annihilate or create edges (noted explicitly by Bidarra and Bronsvoort). Naming faces is comparatively tractable because faces usually correspond to feature surfaces with clear provenance.

---

## 1.2 Foundational naming schemes

### Kripac (1995 / 1997): the Topological ID System

**Citation.** Kripac, J. (1995). A mechanism for persistently naming topological entities in history-based parametric solid models. In *Proceedings of the Third ACM Symposium on Solid Modeling and Applications (Solid Modeling '95)*, Salt Lake City, pp. 21-30. Journal version: Kripac, J. (1997). A mechanism for persistently naming topological entities in history-based parametric solid models. *Computer-Aided Design, 29*(2), 113-122.

**Mechanism.** Kripac introduced the Topological ID System (TIS), the first complete treatment. Each topological entity receives an ID. When the model is edited and automatically re-evaluated from its sequential history, the TIS maps IDs of the old model onto IDs of the corresponding entities of the new model. The scheme is **face-based**: faces are the primary named entities because each face derives from a known generating surface of a feature, giving it durable provenance. Lower-dimensional entities are named indirectly: an edge is named as the intersection of its two adjacent faces, and a vertex as the meeting of its adjacent faces/edges, so their names are derived from the (more stable) face names plus adjacency. Kripac records a **face adjacency graph** so that when a name is ambiguous (several edges arise from the same face pair, e.g. a face that wraps around and intersects another twice) the system can disambiguate. The disambiguation device is **parametric-space information**: the location of the entity in the parametric (u,v) space of its underlying surface is stored and used to pick the correct candidate among entities sharing the same combinatorial name. Implemented in the InteSolid modeler.

**Failure modes.** Parametric-space disambiguation is fragile when the underlying surface itself is reparametrized or replaced on re-evaluation, because the stored (u,v) coordinates no longer point at the same physical location. Splits that produce more than two children, and merges, stress the face-pair naming because the "same" combinatorial name now matches several or zero entities. The scheme presumes deterministic, repeatable re-evaluation; nondeterministic face ordering defeats it.

**Kernel relevance.** Establishes the core requirement Keel must satisfy: the kernel must let a naming layer record, per face, its generating surface/feature provenance, and per edge/vertex its bounding faces, plus a stable parametric-space anchor. Faces should be the unit of identity; everything else is derived.

### Capoyleas, Chen, Hoffmann (1996): generic naming, and Chen & Hoffmann name matching

**Citation.** Capoyleas, V., Chen, X., & Hoffmann, C. M. (1996). Generic naming in generative, constraint-based design. *Computer-Aided Design, 28*(1), 17-26. Companion / context: Chen, X., & Hoffmann, C. M. (1995). On editability of feature-based design. *Computer-Aided Design, 27*(12), 905-914. Hoffmann, C. M. (1993). Semantic problems of generative, constraint-based design. Purdue CS tech report.

**Mechanism.** The setting is *generative* design where the user graphically picks shape elements with *generic intent* (the pick should mean "the face produced by this operation", not "this particular instance"). The scheme names every entity by the **generative operation that created it** plus the operands. Faces are named by the feature/CSG primitive whose surface they lie on. Edges and vertices are named as **intersections of named faces** (an edge = ordered pair of face names; a vertex = the faces meeting there). When an operation splits a parent face into children, all children inherit the parent's name and are distinguished by an **ordinal / qualifier**, so the lineage to the ancestor face is preserved. The companion matching work supplies the algorithm that, on edit, matches the stored generic names against the entities of the regenerated model, walking the same operation order so that names are produced in a comparable sequence.

**Failure modes.** Feature suppression and reordering break the assumption that the operation sequence (and thus the produced names) is stable: a suppressed feature removes the operation that minted a name, orphaning references. Ambiguity when the same face pair yields multiple edges requires extra geometric qualifiers the basic scheme does not fully resolve.

**Kernel relevance.** Cements the "name = operation + operands, with children inheriting ancestor identity" pattern that every later scheme and every shipping kernel (FreeCAD's element map, OnShape's `qCreatedBy`) reuses. Keel's operation layer must tag every output entity with the operation id and the operand entity ids.

### Marcheix & Pierra (2002): a survey and an invariant/topological-history framework

**Citation.** Marcheix, D., & Pierra, G. (2002). A survey of the persistent naming problem. In *Proceedings of the Seventh ACM Symposium on Solid Modeling and Applications*, pp. 13-22.

**Mechanism.** The survey formalizes the naming / name-matching split and proposes a framework that names entities from the **invariant structure of each class of form feature** together with that feature's **topological evolution**, recorded as a **topological history**. Rather than relying on a single snapshot, both the initial and the re-evaluated models carry a topological history (the record of how each entity arose through the operations); matching is performed by **comparing the two topological histories** so that splits and merges are recognized as history events rather than as unexplained appearances/disappearances. This shifts the problem from "match two static B-reps" to "match two derivation histories", which is far more robust to topology change.

**Failure modes.** Storing and comparing full histories is costlier in space and time. Matching becomes ambiguous when the histories diverge structurally (an edit that removes a feature changes the very history that the matcher walks).

**Kernel relevance.** The "topological history" idea is the conceptual ancestor of FreeCAD's history-encoded names and Cascaval's lineage graph. It tells Keel that the durable artifact is not a name string but a **per-entity derivation record** that the kernel emits and the application persists.

### Baba-Ali & Marcheix: edge matching for non-planar intersections

**Citation.** Baba-Ali, M., Marcheix, D., & Skapin, X. (2009 and related). An edge matching technique for non-planar face intersections in geometric parametric models. (Semantic Scholar listing; part of the Marcheix/Baba-Ali line of work circa 2003-2010.) See also Cardot, Baba-Ali, Marcheix et al., persistent naming via graph transformation rules (below).

**Mechanism.** Treats edge matching as a **geometric transformation** problem: rather than relying only on combinatorial face-pair names, it overlays the edges of the initial model onto the edges of the re-evaluated model and uses the geometric correspondence to decide matches, specifically targeting edges produced by intersection of non-planar faces where parametric-space anchors are unreliable. Global and local matching variants are defined (global compares whole histories; local repairs a neighborhood).

**Failure modes.** Geometric overlay assumes the re-evaluated geometry is close to the original; large parameter changes that move edges substantially defeat the overlay heuristic.

**Kernel relevance.** Confirms that a robust naming layer needs both combinatorial provenance (from the kernel) and geometric query access (point/curve evaluation, projection, closest-entity) to break ties. Keel must expose cheap geometric proximity queries on named entities.

### Cardot / Baba-Ali / Marcheix: persistent naming via graph-transformation rules

**Citation.** Persistent naming system based on graph transformation rules to re-evaluate parametric specifications. *Computer-Aided Design and Applications, 16*(5), 985-1002 (2019); HAL hal-01883832.

**Mechanism.** Models the topological history as a **graph** and defines, for each modeling operation, a **graph-transformation rule** describing how the operation rewrites the topology graph (which nodes are created, deleted, split, merged). Names are read off the graph; matching re-applies the rules to the edited specification and compares graphs. This makes split/merge first-class graph events.

**Failure modes.** Requires a complete, formally specified rule per operation, which is heavy engineering for a kernel with many local operations (blends, drafts, shells, sweeps). Graph comparison cost grows with model size.

**Kernel relevance.** A formal target for what "split/merge event reporting" should contain: enough to rewrite a topology graph deterministically. Even if Keel does not adopt graph rewriting, every operation must report its create/delete/split/merge effects in this vocabulary.

---

## 1.3 Alternative philosophies: stop naming non-persistent geometry

### Bidarra & Bronsvoort: the Semantic Feature Model (SFM)

**Citation.** Bidarra, R., & Bronsvoort, W. F. (2000). Semantic feature modelling. *Computer-Aided Design, 32*(3), 201-225. Persistent-naming-specific: Bidarra, Nyirenda & Bronsvoort (2005). A feature-based solution to the persistent naming problem. *Computer-Aided Design and Applications, 2*(1-4), 517-526.

**Mechanism.** Diagnoses the root cause: the persistent-naming problem exists because parametric definitions reference **non-persistent geometric model entities** (the very faces and edges that re-evaluation regenerates). The cure is to reference **persistent parametric entities** instead. The Semantic Feature Model is a *declarative* model: a set of features plus explicit constraints between them, maintained on a **cellular model** (a non-manifold cellular decomposition where each cell carries durable identity and explicit incidence). References are expressed against cells and feature faces rather than against re-derived B-rep faces, so they need not be rematched after every regeneration. An implementation maintains the semantics of the parametric definition across edits. The authors are blunt that existing heuristic naming schemes "solve only a fraction of the problem", with b-rep edges the hardest because a small parameter change can drastically change intersection topology.

**Failure modes.** Requires a richer underlying representation (cellular / non-manifold) than a plain B-rep, raising kernel complexity. Does not by itself make legacy geometry-referencing features robust; it changes what you are allowed to reference.

**Kernel relevance.** Strategic guidance for Keel: prefer a representation where references attach to durable cells/feature-surfaces, and treat raw B-rep faces/edges as derived, queryable views. A cellular/non-manifold-capable topology layer is the structural enabler. This is exactly the "design before freezing topology" warning.

### Mun & Han: OSI + Secondary Name (topology-based basic naming with ambiguity solving)

**Citation.** Mun, D., & Han, S. (2005-2006). Identification of topological entities and naming mapping for parametric CAD model exchanges. *International Journal of CAD/CAM*; and An OSI- and SN-based persistent naming approach for parametric CAD model exchange. *Korean Journal of Computational Design and Engineering*. Context: macro-parametric CAD exchange line of work, KAIST.

**Mechanism.** Two parts. (a) **Naming**: each entity gets a basic name from its generating topology plus, where the basic name is not unique, a **Secondary Name (SN)** and **Object Space Information (OSI)**. OSI uses **object (3D model) space** coordinates instead of parametric (u,v) space to compare entities that share a basic name, which is more robust than Kripac's parametric-space anchor because object-space position survives reparametrization. (b) **Name matching** under that scheme for CAD-to-CAD exchange, driven by XML neutral macros that retain design intent. Farjana & Han's review recommends this OSI-based ambiguity solving combined with topology-based basic naming as a de-facto standard.

**Failure modes.** Object-space coordinates still move under large parameter edits; OSI is a tie-breaker, not a guarantee. Exchange-oriented, so it assumes the macro history is portable.

**Kernel relevance.** Argues for storing an **object-space anchor** (a representative 3D point on each named entity) alongside combinatorial provenance, as a robust disambiguator. Cheap for Keel to provide and it survives reparametrization better than (u,v).

### Wang et al. (Georgia Tech): geometry-based semantic ID

**Citation.** Wang et al. Geometry-based semantic ID for persistent and interoperable reference. *Journal of Computer-Aided Design* / Georgia Tech MSSE (msse.gatech.edu/publication/JCAD_PID_wang.pdf).

**Mechanism.** Builds an identifier anchored to **geometric characteristics** of the entity plus its topological relationships, organized in a namespace that keeps semantic meaning across re-evaluation and, crucially, across *different CAD systems* (interoperability). It leans on Raghothama-Shapiro **BR-variance**: as long as a feature's geometry stays continuous under parameter change, its geometry-derived id is stable. A "naming service" component does collision detection; explicit mappings are kept across re-evaluations.

**Failure modes.** Splits/merges break one-to-one id correspondence; geometric discontinuity violates the BR-variance assumption; two entities with identical geometric signatures collide and may need user intervention.

**Kernel relevance.** Reinforces the object-space-anchor idea and the value of a stable geometric signature per entity. For Keel's interoperability story (STEP import/export), a geometry-derived id is a useful complement to history-derived names.

### Raghothama & Shapiro: BR-variance, the formal foundation

**Citation.** Raghothama, S., & Shapiro, V. (1998). Boundary representation deformation in parametric solid modeling. *ACM Transactions on Graphics, 17*(4), 259-286. Related: Necessary conditions for boundary representation variance; Consistent updates in dual representation systems, *Computer-Aided Design* (2000).

**Mechanism.** Provides the *theory* the heuristic naming schemes lack. Formulates **BR-variance**: two solids belong to the same parametric family and admit a consistent boundary correspondence if the boundary deforms continuously under the parameter change. Gives necessary (and, in restricted cases, sufficient) conditions for a B-rep update to be **valid** (the new B-rep genuinely models the same parametric family). This lets a system *detect invalid updates* and gives "a formal criterion for the recently proposed heuristic approaches to the persistent-naming problem."

**Failure modes.** The continuity assumption is exactly what topology-changing edits violate; the theory characterizes when naming *can* work, not a procedure that always works through topology change.

**Kernel relevance.** Gives Keel a validity oracle: after a re-evaluation, the kernel can flag whether the boundary deformation was continuous (names trustworthy) or whether a topology event occurred (names must be rematched and possibly surfaced to the user). Worth exposing a per-operation "topology changed?" / continuity signal.

---

## 1.4 Naming in shipping systems

### FreeCAD: the topological naming problem, and the realthunder / Ondsel element-map fix

**Citation.** FreeCAD documentation, *Topological naming problem* (FreeCAD/FreeCAD-documentation wiki). realthunder, Topological Naming Algorithm (asm3-wiki) and FreeCAD PRs #4752, #7427. Ondsel engineering blog (2023-2024) on the toponaming fix phases.

**Mechanism.** Baseline FreeCAD names elements as **type + index** (`Face13`, `Edge2`), where the index comes from the OCCT kernel's iteration order. When an upstream sketch/pad changes, OCCT renumbers, so `Face13` becomes `Face14`; a downstream sketch still bound to `Face13` now attaches to the wrong (side) face and the feature fails. The fix (realthunder, productized by Ondsel) replaces index names with **history-encoded MappedNames** computed from OCCT's `BRepBuilderAPI_MakeShape` history (`Modified` / `Generated` / `Deleted`). A name encodes the source element name, a marker (`;:M` modified, `;:G` generated, `;:MG`, `;:U` derived-from-upper, `;:L` constructed-from-lower), the operation code (e.g. `FUS` for fuse), and tag/length/type metadata. Example: `Face6;:M2;FUS;:T1:5:F`. Multi-source merges nest sources in parentheses: `Face6;:M(Face2;:T2:5:F);FUS;:T1:5:F`. Splits get per-child indices `;:G1`, `;:G2`. A **StringHasher** compresses runaway names to hex ids (`#a8;:M#a7;RFI;:T2:2:F`). `getElementHistory()` decodes a name back through the operation chain (Fillet -> Fusion -> Cylinder); `getRelatedElements()` does fallback recovery by prefix-matching modified versions and siblings-from-same-source. The official docs stress the fix has three jobs: detect broken references and show an error, suggest a candidate fix for user approval, and auto-repair only with high confidence (a wrong auto-repair re-introduces the bug).

**Failure modes.** Names grow large (mitigated by the hasher); measured overhead on a 148-object model was +30% recompute time and +27-29% file size. The fix reduces but does not eliminate the need for careful modeling (datums, explicit placement); ambiguous cases still require user choice.

**Kernel relevance.** This is the most concrete, reproducible blueprint available, and it is a *retrofit onto a kernel (OCCT) that already exposes Modified/Generated/Deleted history*. The lesson for Keel: expose that history natively and cheaply from the start, and design names/ids so they can be compressed (interned/hashed) because they compound.

### OnShape / FeatureScript: queries and enforced determinism

**Citation.** Onshape FeatureScript documentation (cad.onshape.com/FsDoc): Introduction, Standard Library; Onshape developer docs on evaluating FeatureScript; Onshape blog on flexible parametric models and the Repair tool.

**Mechanism.** OnShape sidesteps stored names by making references **deferred queries**. A `Query` is "a map that contains instructions for how to find entities", i.e. an order form with criteria, not a pointer. Examples: `qCreatedBy(id, EntityType.FACE)` (entities created by a given operation id), composed with set operators (`qUnion`, etc.). A query is only resolved by `evaluateQuery(context, query)` **against the current geometry at the moment it is needed**, so the same query adapts as the model regenerates. References are thus expressed as construction-history criteria ("the faces created by extrude1") rather than indices. This rests on a hard guarantee: "Determinism is a core principle of FeatureScript. Models must regenerate the same way every time, everywhere." FeatureScript has no undefined behavior, no influence from time/randomness/external input, value semantics everywhere, and explicit boxes for the rare mutable reference. When references still break, the Repair tool shows current vs last-known-healthy geometry for manual rebinding.

**Failure modes.** A query can match *more or fewer* entities than intended after an edit (a fillet query that grabs an unexpected extra edge); `qCreatedBy` does not always behave as users expect across Booleans. Determinism shifts the burden onto the kernel: every operation, iteration, and tolerance decision must be reproducible.

**Kernel relevance.** Two mandates for Keel: (1) make every entity tag its creating operation id and operand provenance so a `qCreatedBy`-style query is answerable; (2) **guarantee deterministic re-evaluation** (stable iteration order, deterministic entity ids, deterministic tolerance/numerical decisions). Without kernel determinism, no naming or query scheme is sound. Rust helps here: no data races, controllable iteration order, no hidden global mutable state.

### Cascaval, Bodik & Schulz: lineage-based referencing DSL

**Citation.** Cascaval, D., Bodik, R., & Schulz, A. (2023). A lineage-based referencing DSL for computer-aided design. *Proceedings of the ACM on Programming Languages, 7*(PLDI), 76-99. Related: Cascaval et al. (2022), Differentiable 3D CAD programs for bidirectional editing, *Computer Graphics Forum, 41*. Implementation: github.com/dcascaval/lineage-based-cad-referencing.

**Mechanism.** Introduces **lineage**: a constructed graph of the model's computational history, where each geometric element records which prior elements it was derived from. References are written as **queries over lineage relations** rather than picks or indices, making them robust to *both* parameter edits and *program* edits (insert/remove/reorder operations). The DSL provides operators such as `from(geo)` (elements originating directly from a geometry), `derivedFromAny(...)` and `derivedFromAll(...)` (elements whose construction involved any/all of the given sources), combined with boolean/set operators. Example syntax: `query(a.polygons, from(f) and not(from(g)))`, `query(e.points, derivedFromAll(f1, f2))`. Because the predicate is over durable lineage relations, it re-resolves correctly as the model changes.

**Failure modes.** The differentiable/editing companion explicitly "is not designed to optimize across changes in geometric topology"; lineage robustly *identifies* across edits but does not invent semantics when an element's lineage genuinely ceases to exist.

**Kernel relevance.** The strongest modern formalization of the OnShape/FreeCAD intuition. It tells Keel that the right primitive the kernel must feed upward is a **lineage edge per output entity (this came from these inputs via this op)**, and that the application-layer reference language should be a query over that lineage, not a stored index. This is the recommended design for Keel's reference model.

---

# PART 2: Sessions, rollback, journaling

## 2.1 Parasolid's documented session model

### Partitions, pmarks, and deltas

**Citation.** Siemens (formerly UGS/EDS) Parasolid documentation, *Partitions and Rollback*; *Rollback*; reference pages `PK_PARTITION_receive_deltas`, `PK_PMARK_goto`, `PK_PMARK_ask_partition`, `PK_MARK_goto_2`, `PK_PARTITION_goto_2` (public mirror q-solid.com Parasolid v12.0 and V35 docs; Siemens PLM docs).

**Mechanism.** A **partition** is a sub-division of a session that groups bodies and can be rolled, transmitted, and locked independently of other partitions. A **pmark** (partition rollmark) is a saved point in a partition's history; the application sets pmarks and later rolls a partition to any of them. State change between adjacent pmarks is captured as a **delta**: "a delta holds the changes required to get from one pmark to an adjacent one." Deltas are **unidirectional**, holding only enough to go from pmark A to pmark B, not back. When Parasolid executes a roll from one pmark to an adjacent one, it **creates a new reversed delta** on the fly to provide the return route, so undo/redo is symmetric without storing both directions up front. Two granularities exist: **session-level rollback** rolls the whole session's contents together; **partition-level rollback** rolls one partition independently. Rollback must be enabled before the session starts by registering a **delta frustrum** (the application-supplied storage callback); each delta (rollback file) gets a positive integer id allocated by the frustrum. `PK_PARTITION_receive_deltas` moves delta state between sessions. Roll-forward is opt-in: if `roll_forward` is false when a partition is finished with, the partition and all its pmarks are deleted.

**Failure modes.** Delta storage grows with edit count; the application must garbage-collect old pmarks. Journal files saved from legacy versions can break because face/edge/curve ids changed, so deltas/journals are version-bound.

**Kernel relevance.** This is the reference design for Keel's session layer. Concrete takeaways: model state-transitions as **directed deltas** with on-demand reversal rather than storing full snapshots; support **independent roll units** (Keel partitions) so a large assembly need not roll wholesale; make storage **pluggable** (a frustrum-equivalent) so embedders control persistence; gate rollback enablement at session start because it changes how every op records state.

### Error recovery, transactionality, and atomic operations

**Citation.** Parasolid documentation, *Error Handling*; *Checking*; behavior notes for `PK_PMARK_goto_2` / `PK_MARK_goto_2`.

**Mechanism.** PK errors are graded **mild / serious / fatal**, and applications are "strongly advised to implement rollback" to restore the session after serious errors. Rollback is the primary recovery primitive: if a PK call detects an error (e.g. a Boolean failure), the caller rolls back to a known-good pmark. The system guarantees a **known-state invariant** even mid-failure: if memory is exhausted or the frustrum returns a write error during `PK_PMARK_goto_2`/`PK_MARK_goto_2`, some partitions may have rolled partway, "however... the session is in a known state (all partitions are at some pmark)", so after freeing resources the application can retry. Failing to roll back after a modeling error is documented as indicating either a serious Parasolid bug or an invalid PK call sequence, i.e. rollback is the contract for recovering atomicity.

**Failure modes.** Atomicity is achieved by convention (caller must roll back), not automatically per call; a caller that ignores the protocol leaves a corrupt model. Partial rolls mid-failure require retry logic.

**Kernel relevance.** Keel should make failed operations **automatically atomic**: a failed Boolean/local op must leave the model exactly as before, not partially mutated. The Parasolid known-state invariant (always at *some* checkpoint, even after a failed roll) is a design goal. Rust's ownership and `Result`-based error flow make "build the result, then commit or drop" natural: a copy-on-write topology lets a failed op simply drop its work-in-progress.

### Journaling for deterministic replay and debugging

**Citation.** Parasolid documentation, *Introduction to Parasolid*; journal-file description; *Using Reports*.

**Mechanism.** Parasolid can record **every PK function call to an external journal file**, capturing the arguments supplied and results returned. Replaying the journal reproduces the session deterministically, which is the standard mechanism for reproducing and debugging customer bugs: the customer ships the journal, the vendor replays it. `PK_SESSION_receive` restores a transmitted snapshot into the same Parasolid version, complementing journals for state recovery.

**Failure modes.** Journals are version- and id-bound (legacy journals break when ids change); they can be large; they require strict determinism to replay faithfully.

**Kernel relevance.** Keel should support **call journaling** from day one: a record/replay log of kernel operations is the single highest-leverage testing and debugging tool (regression capture, fuzzing corpus, customer bug repro). It only works if the kernel is deterministic, tying back to the OnShape mandate.

### Concurrency and thread safety

**Citation.** Parasolid documentation, *Calling Parasolid From Multiple Threads*.

**Mechanism.** Parasolid classifies PK functions as **concurrent** (multiple threads may run them simultaneously), **exclusive** (no other thread may be inside Parasolid while one runs), or **locally exclusive** (exclusive within a thread that has locked partitions; behaves as exclusive when no partitions are locked; otherwise concurrent/exclusive depending on whether the caller is a locking thread). The unit of concurrency is the **locked partition**: a thread locks one or more partitions to itself, then operates on them concurrently with other threads working on other partitions. This is coarse-grained, partition-level locking, not fine-grained per-entity locking.

**Failure modes.** Coarse locking limits parallelism within a single partition; the concurrent/exclusive taxonomy is intricate and easy to misuse.

**Kernel relevance.** A major Rust opportunity. Where Parasolid relies on documented call discipline and partition locks enforced at runtime, Keel can encode the same model in the **type system**: `&mut Partition` gives exclusive access checked at compile time, `&Partition` allows shared concurrent reads, and `Send`/`Sync` bounds make "this operation may cross threads" a compiler-verified property. Per-partition ownership maps cleanly to Rust borrows, eliminating an entire class of the concurrency bugs Parasolid's manual rules exist to prevent.

---

## 2.2 ACIS history streams and bulletin boards

**Citation.** Spatial ACIS documentation, *Model Management > Bulletin Boards*; *Bulletin Board APIs*; *roll:merge-delta-states* (q-solid ACIS R17 mirror; Arizona ISL ACIS docs). APIs: `api_note_state`, `api_change_state`, `api_change_to_state`, `api_find_named_state`, `api_name_state`, `api_roll_n_states`, `api_delete_ds`. Macros: `API_BEGIN`/`API_END`, `API_NOP_BEGIN`/`API_NOP_END`, `API_TRIAL_BEGIN`/`API_TRIAL_END`.

**Mechanism.** ACIS records change at entity granularity via **bulletins**. A `BULLETIN` holds an **old entity pointer and a new entity pointer** plus a type (create / delete / change), so it records exactly what an operation did to one entity. Bulletins accumulate on a **bulletin board**, which collects all entity changes for one logical operation. A completed bulletin board becomes a **delta state**, and delta states chain into the **history stream**; rolling back/forward navigates the stream by applying/inverting delta states. For history and roll to work, model-changing code must be wrapped in `API_BEGIN`/`API_END` (which open/close a bulletin board); trial macros allow speculative work that can be discarded. **Named states** (`api_name_state` / `api_find_named_state`) mark points to return to (the equivalent of Parasolid pmarks). `api_roll_n_states` rolls N steps; `api_delete_ds` prunes. When `api_logging` is FALSE, ACIS "discards all but the current delta state", trading away undo for memory.

**Failure modes.** Forgetting the `API_BEGIN`/`API_END` wrapping leaves changes untracked and breaks roll. Unbounded history streams consume memory; pruning is the application's job. Entity-pointer-based bulletins are inherently in-memory and pointer-bound.

**Kernel relevance.** ACIS gives Keel a second, finer-grained reference design: **per-entity change records (old, new, kind)** grouped into a per-operation transaction, chained into a navigable history. Compared with Parasolid's opaque deltas, the bulletin model is more transparent and maps well onto a copy-on-write/persistent topology where "old" and "new" are simply two immutable versions. Named states confirm the need for application-settable checkpoints.

---

## 2.3 Undo/redo architectures and immutable representations

**Citation.** Driscoll, Sarnak, Sleator & Tarjan (1989), Making data structures persistent, *Journal of Computer and System Sciences, 38*(1) (foundational). Practitioner syntheses: Wikipedia, *Persistent data structure*; SoftwareMill and August Lilleaas, persistent data structures in functional programming. Applied context: CAD/editor undo via structural sharing.

**Mechanism.** Two broad architectures. (1) **Command pattern / journal of operations**: store the operation log; undo by inverse operations or by replay from a checkpoint. This is Parasolid's delta/journal and ACIS's delta-state model. (2) **State deltas over a persistent (immutable) data structure**: a persistent data structure "always preserves the previous version of itself when modified", yielding a new version while sharing unchanged substructure (**structural sharing**); copy-on-write is the simplest instance. Undo/redo, branching histories, and snapshots fall out for free because every version remains addressable. Persistent trees copy only the root-to-modified-node path; the rest is shared. These structures are explicitly cited as the backbone of editor/CAD undo and branching operation histories.

**Failure modes.** Naive immutability copies too much (mitigated by structural sharing); pointer-chasing through shared nodes can hurt cache locality; lifetime/aliasing management is delicate in non-GC languages.

**Kernel relevance.** This is the cleanest fit for a Rust kernel and is recommended for Keel. A **copy-on-write / persistent B-rep topology** gives: (a) free undo/redo and named checkpoints (keep handles to old versions), (b) automatic transactional atomicity (a failed op drops its new version, the old version is untouched), (c) cheap snapshots for journaling and for the "last-known-healthy" model OnShape's Repair tool shows. Rust's `Arc` + structural sharing implement persistent topology idiomatically, and the borrow checker prevents accidental mutation of a shared prior version. This also dovetails with naming: an entity's lineage is naturally expressed as edges between immutable versions.

---

# Naming and session design mandates for Keel

The literature and the two industrial designs converge on a small set of **kernel hooks that must exist before the topology layer is frozen**. These are non-negotiable because each is structural.

**Naming (must be emitted by every operation, from day one):**

1. **Per-output-entity lineage records.** Every face/edge/vertex produced by an operation must carry: the **creating operation id**, the **operand entity ids it descends from**, and a **derivation kind** in the vocabulary {created, modified, generated, split-child, merge-result, deleted}. This is the common denominator of Kripac, Capoyleas/Chen/Hoffmann, Marcheix, FreeCAD's MappedName, OnShape's `qCreatedBy`, and Cascaval's lineage. It is the single most important hook.
2. **Explicit Modified / Generated / Deleted reporting per operation.** Booleans and every local op (fillet, chamfer, shell, draft, sweep, pattern) must return the full input-to-output mapping, exactly as OCCT's `BRepBuilderAPI_MakeShape` history does and as FreeCAD's fix consumes. No operation may produce anonymous topology.
3. **First-class split and merge events.** When one entity becomes many (or many become one), the report must say so and link all children/parents to the ancestor(s), with an ordinal to distinguish split children. This is where every naive scheme fails.
4. **Stable disambiguation anchors.** Store, per entity, an **object-space representative point** (Mun/Han OSI, more robust than parametric (u,v)) and optionally a parametric-space anchor (Kripac) for tie-breaking when combinatorial names are ambiguous.
5. **Deterministic re-evaluation.** Stable iteration order over topology, deterministic entity-id assignment, and deterministic numerical/tolerance decisions. Without this, no naming or query scheme is sound (OnShape's hard rule). Rust aids determinism: no data races, explicit ordering, no hidden global state.
6. **References as queries over lineage, not stored indices.** Keel's reference model should be a lineage query language (`from`, `derivedFromAny`, `derivedFromAll`, `createdBy`, set combinators), resolved against the current model at use time. Stored string names should be interned/hashed (FreeCAD's StringHasher) because they compound.
7. **A continuity / topology-change signal.** Per Raghothama-Shapiro BR-variance, the kernel should report whether a re-evaluation deformed the boundary continuously (names trustworthy) or underwent a topology event (rematch required, possibly surface to user). This is the validity oracle.
8. **Prefer durable cells over raw B-rep entities for references.** Bidarra-Bronsvoort's lesson: design the topology layer so references can attach to persistent cells/feature-surfaces, treating raw B-rep faces/edges as derived views. This argues for a cellular/non-manifold-capable topology, decided now.

**Sessions, rollback, journaling (must be in the architecture, not retrofitted):**

9. **Copy-on-write / persistent topology** as the substrate for undo/redo, checkpoints, and atomicity (persistent-data-structure literature; natural in Rust via `Arc` + structural sharing).
10. **Checkpoints (pmark/named-state equivalent) and directed deltas with on-demand reversal** (Parasolid model); or equivalently addressable immutable versions (ACIS delta-state / persistent-structure model). Either way: application-settable named restore points.
11. **Independent roll units (partitions)** so large models roll selectively, with **pluggable delta storage** (Parasolid frustrum) controlled by the embedder.
12. **Automatic transactional atomicity.** A failed operation must leave the model byte-identical to before (Parasolid's known-state invariant). Copy-on-write makes this free: drop the work-in-progress version.
13. **Call journaling (record/replay) from day one.** Deterministic replay of the kernel call log for regression capture, fuzzing, and customer bug reproduction (Parasolid journal files). Depends on mandate 5.
14. **Compile-time concurrency model.** Encode Parasolid's concurrent/exclusive/locally-exclusive partition model in Rust's type system: `&mut Partition` for exclusive mutation, `&Partition` for shared concurrent reads, `Send`/`Sync` bounds for cross-thread operations. This turns Parasolid's runtime locking discipline into compiler-checked guarantees, Keel's clearest concurrency advantage.

---

# References

1. Kripac, J. (1995). A mechanism for persistently naming topological entities in history-based parametric solid models. *Proc. Third ACM Symposium on Solid Modeling and Applications*, 21-30. https://www.semanticscholar.org/paper/a5e40e5ac256c3d3c9beb7678ab9ffe6ac3d1e6f
2. Kripac, J. (1997). A mechanism for persistently naming topological entities in history-based parametric solid models. *Computer-Aided Design, 29*(2), 113-122. https://www.sciencedirect.com/science/article/abs/pii/S0010448596000401
3. Capoyleas, V., Chen, X., & Hoffmann, C. M. (1996). Generic naming in generative, constraint-based design. *Computer-Aided Design, 28*(1), 17-26. https://www.cs.purdue.edu/cgvlab/www/resources/papers/Capoyleas-Computer_aided_design-1996-Generic_naming_in_generative.pdf
4. Chen, X., & Hoffmann, C. M. (1995). On editability of feature-based design. *Computer-Aided Design, 27*(12). https://www.cs.purdue.edu/cgvlab/www/resources/papers/Chen-Computer_aided_Design-1995-On_editability_of_feature-based_design.pdf
5. Hoffmann, C. M. (1993/2005). Semantic problems of generative, constraint-based design; Constraint-based computer-aided design. Purdue CS. https://www.cs.purdue.edu/cgvlab/www/resources/papers/Hoffmann-Citeseer-2005-Constraint_based_computer_Aided_Design.pdf
6. Marcheix, D., & Pierra, G. (2002). A survey of the persistent naming problem. *Proc. Seventh ACM Symposium on Solid Modeling and Applications*, 13-22. https://www.researchgate.net/publication/221115805_A_survey_of_the_persistent_naming_problem
7. Baba-Ali, M., & Marcheix, D. An edge matching technique for non-planar face intersections in geometric parametric models. https://www.semanticscholar.org/paper/55bba6f06148a87beb88105cf6c0b8d9cd7a3913
8. Cardot, Baba-Ali, Marcheix et al. (2019). A persistent naming system based on graph transformation rules to re-evaluate parametric specification. *Computer-Aided Design and Applications, 16*(5), 985-1002. https://cad-journal.net/files/vol_16/CAD_16(5)_2019_985-1002.pdf ; HAL: https://hal.science/hal-01883832
9. Bidarra, R., & Bronsvoort, W. F. (2000). Semantic feature modelling. *Computer-Aided Design, 32*(3), 201-225.
10. Bidarra, R., Nyirenda, P. J., & Bronsvoort, W. F. (2005). A feature-based solution to the persistent naming problem. *Computer-Aided Design and Applications, 2*(1-4), 517-526. https://graphics.tudelft.nl/~rafa/myPapers/jrnl-bidarra.CAD05.pdf
11. Farjana, S. H., & Han, S. (2018). Mechanisms of persistent identification of topological entities in CAD systems: A review. *Alexandria Engineering Journal, 57*(4), 2837-2849. https://www.sciencedirect.com/science/article/pii/S1110016818300814
12. Mun, D., & Han, S. (2005-2006). Identification of topological entities and naming mapping for parametric CAD model exchanges; OSI- and SN-based persistent naming approach. *Int. J. CAD/CAM*; *Korean J. Computational Design and Engineering*. https://www.koreascience.or.kr/article/JAKO200503018236985.page
13. Wang et al. Geometry-based semantic ID for persistent and interoperable reference. Georgia Tech MSSE / J. Computer-Aided Design. https://msse.gatech.edu/publication/JCAD_PID_wang.pdf
14. Raghothama, S., & Shapiro, V. (1998). Boundary representation deformation in parametric solid modeling. *ACM Transactions on Graphics, 17*(4), 259-286. https://dl.acm.org/doi/10.1145/293145.293148
15. Raghothama, S., & Shapiro, V. (2000). Consistent updates in dual representation systems. *Computer-Aided Design*. https://www.sciencedirect.com/science/article/abs/pii/S0010448500000361
16. FreeCAD documentation. Topological naming problem. https://github.com/FreeCAD/FreeCAD-documentation/blob/main/wiki/Topological_naming_problem.md
17. realthunder. Topological Naming Algorithm (asm3-wiki); FreeCAD PRs #4752, #7427. https://github.com/realthunder/asm3-wiki/blob/master/Topological-Naming-Algorithm.md
18. Ondsel. Toponaming fix milestone blog posts. https://www.ondsel.com/blog/milestone-toponaming-fix-phase-2-done/
19. Onshape. FeatureScript documentation: Introduction; Standard Library; evaluating FeatureScript. https://cad.onshape.com/FsDoc/intro.html ; https://onshape-public.github.io/docs/api-adv/fs/
20. Cascaval, D., Bodik, R., & Schulz, A. (2023). A lineage-based referencing DSL for computer-aided design. *Proc. ACM Programming Languages, 7*(PLDI), 76-99. https://dcascaval.github.io/lineage-based-cad-referencing-pldi23.pdf ; https://github.com/dcascaval/lineage-based-cad-referencing
21. Cascaval, D., et al. (2022). Differentiable 3D CAD programs for bidirectional editing. *Computer Graphics Forum, 41*. https://arxiv.org/abs/2110.01182
22. Siemens Parasolid documentation. Partitions and Rollback; Rollback; Error Handling; Calling Parasolid From Multiple Threads; journal files; PK_PARTITION_receive_deltas, PK_PMARK_goto. https://www.plm.automation.siemens.com/en_us/products/open/parasolid/support/documentation.shtml ; mirror: http://www.q-solid.com/Parasolid_Docs/chapters/fd_chap.42.html
23. Spatial ACIS documentation. Model Management > Bulletin Boards; Bulletin Board APIs; delta states and roll. http://www.q-solid.com/ACIS_Docs_R17/online/SPAacisuserTechArticles/SPAacisuser_mhhibb.htm
24. Driscoll, J. R., Sarnak, N., Sleator, D. D., & Tarjan, R. E. (1989). Making data structures persistent. *Journal of Computer and System Sciences, 38*(1), 86-124. Persistent data structures overview: https://en.wikipedia.org/wiki/Persistent_data_structure
