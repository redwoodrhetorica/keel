# 52. Persistent Topological Naming for Keel: the Keel-specific application

## Title and scope

This dossier is the **Keel-specific application** of persistent topological naming. It is not a literature survey. The broad survey already exists in `docs/research/kernel/07-persistent-naming-sessions.md` (Kripac; Capoyleas / Chen / Hoffmann; Marcheix / Pierra; Baba-Ali / Marcheix; Cardot graph-rewriting; Bidarra / Bronsvoort SFM; Mun / Han OSI+SN; Wang geometry-ID; Raghothama / Shapiro BR-variance; FreeCAD / realthunder / Ondsel; OnShape / FeatureScript; Cascaval lineage DSL). **Read file 07 for the literature.** This file answers one question only: **given Keel's existing machinery, how do we build persistent naming and resolve the hard cases.** It assumes file 07's vocabulary and does not re-derive it.

This addresses roadmap items 82-85 (parametric / feature-tree hosts, edit-surviving assembly instances) and item 124. The deliverable gates every host that wants "a reference to the face/edge created in step 3" to survive when a later edit (boolean, tweak, fillet) regenerates the topology.

**Everything here is a design recommendation to validate against Keel's tests**, in particular the box -> chamfer -> hole edit chain in the closing validation section. Where it touches the boolean pipeline it builds directly on dossier 47 (`EdgeId` identity from imprint, the closure invariant, `KeptFace { operand, face, reversed }`, the file-46 sense convention).

### Keel's actual state (the substrate we build on)

Two facts about Keel drive every decision below:

1. **Deterministic `EntityId`, stable WITHIN a single build.** Faces, edges, vertices get ids that are reproducible for a given build but are not assumed to survive across a regeneration. A box face is `Face#7` this build and may be `Face#11` next build. `EntityId` alone is the FreeCAD `Face13`-by-iteration-order trap (file 07, FreeCAD entry): a raw id reference rebinds to the wrong entity on regeneration. So `EntityId` is a within-build handle, never a persistent name.

2. **A `lineage` map.** Keel already records derivation events: per output entity, which operation produced it and which operand entities it descended from. This is mandate 1 of file 07 ("per-output-entity lineage records") already partially in place, and it is the single asset that makes a history-based naming scheme cheap for Keel. The boolean pipeline already carries `EdgeId` identity through imprint (dossier 47). Persistent naming is, for Keel, mostly a matter of **promoting the lineage map into a persistable name and writing the re-resolution replay**, not building new kernel machinery.

The gap: `lineage` today is a within-build derivation record (it links this build's outputs to this build's inputs). It is not yet (a) persistable across edits, (b) replayable against a regenerated build, or (c) equipped with split/merge disambiguation. Closing that gap is this dossier.

---

## Q1. Which canonical approach fits Keel, and why

**Recommendation: a Capoyleas / Chen / Hoffmann generative-history naming scheme, expressed as Cascaval-style lineage queries, with Kripac's face-primary structure and Mun / Han object-space anchors as the disambiguators. NOT Bidarra / Bronsvoort SFM as the primary mechanism.**

The three candidates and why one wins for Keel specifically:

### Kripac (topological-ID matching): adopt the structure, not the mechanism

Kripac names a face by `(step id, face index within step, surface type)` and names edges/vertices indirectly as intersections of adjacent faces, disambiguating with a face-ID graph and a **parametric-space (u,v) anchor** (file 07, Kripac entry; confirmed: faces carry a step id plus per-step face index, edges/vertices derived from adjacent-face names, ambiguity resolved by a face-ID graph). The **structure** is exactly right for Keel and we adopt it: faces are the unit of identity, edges/vertices are named by their bounding faces. But Kripac's matching **mechanism** is a snapshot-to-snapshot ID remap, and its disambiguator is the **parametric anchor, which is fragile under reparametrization** (file 07 failure mode; Kripac's own weakness). Keel reparametrizes surfaces freely (canonical recovery, M8). So we keep Kripac's face-primary name structure and replace his parametric anchor with Mun / Han's **object-space** anchor (a 3D point), which survives reparametrization (file 07, Mun/Han entry).

### Capoyleas / Chen / Hoffmann (generic naming): the right primary scheme

CCH names every entity by the **generative operation that created it plus its operands**: faces by the feature/primitive surface they lie on, edges/vertices as intersections of named faces, and crucially **children of a split inherit the parent's name and are distinguished by an ordinal** (file 07, CCH entry). The matching companion walks the same operation order so names are produced in a comparable sequence. **This is a direct fit for Keel because the name is exactly Keel's lineage edge** (operation + operands + derivation kind). Keel does not have to build a new provenance store; it already emits the lineage. CCH is the scheme whose data requirement Keel already satisfies.

### Bidarra / Bronsvoort (semantic feature model): strategic background, not the primary mechanism

SFM's diagnosis is correct and already absorbed into Keel's architecture: reference durable cells, not re-derived B-rep faces, and Keel is a non-manifold PES kernel precisely so durable cells are representable (file 07 mandate 8). But SFM as a **primary naming mechanism** demands a full declarative feature+constraint model maintained on a cellular decomposition. That is a *host-layer* (feature-tree application) responsibility, not a kernel responsibility, and items 82-85 ask the kernel to make any host's references survive, not to impose one host's feature semantics. So SFM is the strategic frame (prefer durable references), but the concrete kernel mechanism is CCH-over-lineage. Keel offers the durable substrate; the host may build SFM on top if it wishes.

### Why this combination, for a tolerant non-manifold kernel that already has lineage

- **It reuses the lineage map.** CCH's "name = operation + operands" *is* the lineage edge. Zero new provenance machinery; we persist and replay what Keel already records. This is the decisive reason.
- **Tolerant-f64 demands identity-not-coordinate.** Dossier 47's core lesson: Keel cannot fall back on exact re-derivation, so it must never re-derive by coordinate. A history/lineage name is an **identity** match (replay the derivation), with geometry used only as a tie-breaker. Kripac's parametric anchor and any pure-geometry signature (Wang) are tie-breakers, never the primary key, exactly as dossier 47 makes `EdgeId` the primary key and coordinate only a last resort.
- **Non-manifold needs face-primary + radial structure.** Kripac's face-primary structure plus Weiler radial cycles (Keel's PES, dossier 47) means an edge with three or more incident faces is named by its (ordered) bounding-face set, which the radial cycle already records. A manifold-only naming scheme cannot express this; Keel's can.
- **Modern formalization is Cascaval.** CCH-over-lineage, written as a query language (`from`, `derivedFromAny`, `derivedFromAll`, set operators), is precisely Cascaval's lineage DSL (file 07, Cascaval entry). We adopt that surface syntax because it re-resolves robustly under both parameter edits and program edits, which is what items 82-85 need.

**Net: Keel's persistent name is a CCH generative-history name, structured Kripac-style (face-primary, edges by bounding faces), carried as Keel lineage edges, queried Cascaval-style, disambiguated by Mun / Han object-space anchors and a canonical ordering key.**

### Sources for Q1

**Kripac (1997), A mechanism for persistently naming topological entities.**
- Citation: J. Kripac, *Computer-Aided Design* 29(2), 113-122, 1997 (file 07 ref 2).
- Content (confirmed this pass): a face is named by `(step id, face index within that step, surface type)`; edges and vertices are identified by the names of their adjacent faces; ambiguity is resolved with a face-ID graph and intersection codes; on edit the old-model IDs are mapped onto new-model IDs by re-evaluation from history.
- Kernel relevance: gives Keel the **name structure** (face-primary, lower entities by bounding faces). Keel adopts the structure but swaps the parametric (u,v) anchor for an object-space anchor and replaces snapshot-remap with lineage replay.

**Capoyleas, Chen, Hoffmann (1996), Generic naming in generative, constraint-based design.**
- Citation: V. Capoyleas, X. Chen, C. M. Hoffmann, *Computer-Aided Design* 28(1), 17-26, 1996 (file 07 ref 3).
- Content: name = generative operation + operands; split children inherit the parent name plus an ordinal; matching walks the same operation order.
- Kernel relevance: this *is* Keel's lineage edge. The chosen primary scheme because Keel already emits the data it needs.

**Bidarra, Nyirenda, Bronsvoort (2005), A feature-based solution to the persistent naming problem.**
- Citation: *Computer-Aided Design and Applications* 2(1-4), 517-526 (file 07 ref 10).
- Content: reference persistent parametric entities / cells, not re-derived B-rep faces.
- Kernel relevance: the strategic frame and the justification for Keel being non-manifold PES. Demoted to host-layer concern, not the kernel's primary naming mechanism.

---

## Q2. Naming entities by generative history, leveraging Keel's lineage

### The name structure (an operation-relative path of operands and derivation kinds)

A Keel persistent name is **not** an `EntityId`. It is a small **lineage path** rooted at durable anchors (sketch/primitive inputs and operation ids, which the host's feature tree owns and which are stable by construction) and ending at the entity. Structurally it mirrors FreeCAD's MappedName (file 07, FreeCAD entry; confirmed: MappedNames encode operation history, the chain preserves provenance through multiple operations, `getElementHistory()` decodes a name back through the op chain, a StringHasher compresses long names to hashed ids).

A name is a sequence of **derivation steps**, each step:

```
KeelName ::= Anchor ( "/" Step )*
Anchor   ::= FeatureInput(feature_id, role)        // e.g. Sketch3.profile, Box1.primitive
Step     ::= op_id ":" derivation_kind ":" operand_selector [ "#" ordinal ] [ "@" anchor_pt_id ]
derivation_kind ::= Created | Modified | Generated | SplitChild | MergeResult | Copied
operand_selector ::= face_role | bounding_face_set    // faces: which input surface; edges: ordered pair of face-names
```

Reading: "start from a durable feature input, then apply derivation step after derivation step." A face name terminates at a surface-bearing step (which feature surface it lies on). An **edge name terminates at the ordered pair of its two bounding-face names** (Kripac/CCH structure), so edges are named indirectly and inherit the stability of faces. A vertex is named by its meeting faces/edges.

Concretely, for the validation chain:
- The top box face: `Box1.primitive:Created:top`. Durable: it is rooted at the primitive operation and a stable face role, nothing about within-build `EntityId`.
- After a chamfer modifies that face's boundary but not its surface: `Box1.primitive:Created:top / Chamfer1:Modified:top`. The `Modified` step says "the chamfer changed this face's extent but it is the same surface-bearing face." The reference still terminates at the box-top surface.
- A new chamfer face: `Chamfer1:Generated:bevel#0` (the bevel surface the chamfer created; ordinal 0 if the chamfer made several).
- An edge between box-top and a box-side after the chamfer: named by the ordered pair `(Box1...top-after-chamfer, Box1...side-after-chamfer)`, not by any edge `EntityId`.

This is the **OnShape/Cascaval query model grounded in Keel's lineage**: `Box1.primitive:Created:top` is `qCreatedBy(Box1, FACE) filtered to top`; `Chamfer1:Modified:top` is `derivedFrom(that, via Chamfer1)`. Keel stores the path; the host writes it as a Cascaval-style query (`from`, `derivedFromAny`, `derivedFromAll`, set ops, file 07 Cascaval entry, confirmed operator list `from / fromAny / fromAll / derivedFromAny / derivedFromAll / and / or / not / contains`).

### Persisting the name (promoting `lineage` across edits)

Today Keel's `lineage` links **this build's** outputs to **this build's** inputs. To make a name persist:

1. **Root every lineage chain at host-durable anchors, not at `EntityId`s.** When the host references an entity, Keel walks that entity's lineage backward to the nearest **feature-input anchors** (sketch profiles, primitive ops, datum-defined inputs) whose ids the host owns and re-supplies on every regeneration. The persisted name is that backward walk, serialized. The host already keeps feature ids stable (it owns the feature tree); the kernel piggybacks on them.
2. **Intern / hash long names.** Per FreeCAD's StringHasher (file 07; confirmed: strings beyond a threshold are replaced by a SHA1-based integer id), Keel interns each lineage step to a small id and stores the name as a vector of interned step-ids. Names compound through a deep feature tree (chamfer of a fillet of a boolean), so interning is not optional; it is the difference between a 12-byte handle and a kilobyte string. This also gives a fast equality check during replay.
3. **Store one object-space anchor point per named entity** (Mun / Han OSI): a representative 3D point on the entity (face centroid projected to the surface; edge midpoint; vertex position). Persisted alongside the name. It survives reparametrization (unlike Kripac's u,v) and is the tie-breaker of Q3.

### The re-resolution algorithm (replay a name against a regenerated body)

Given a persisted name `N` and a freshly regenerated body `B'` (with a fresh `lineage` map and fresh `EntityId`s):

```
resolve(N, B'):
  1. Bind anchors. Map N's root FeatureInput anchors to B''s current inputs
     by the host-stable feature ids. (Host guarantees these survive; that is
     the contract of items 82-85.) If an anchor's feature was deleted/suppressed,
     report ORPHANED and stop (no auto-rebind to a wrong entity).
  2. Replay the lineage path forward over B''s lineage map. For each Step
     (op_id : kind : selector), find the B' entities that B''s lineage records
     as produced by op_id from the already-bound operand(s), with the matching
     derivation_kind and operand_selector. This is a forward query over the
     SAME lineage relation Keel emits this build; we are matching derivation
     events, not coordinates (Marcheix "compare two topological histories",
     file 07). 
  3. Collect candidates. After the final Step, let C = set of B' entities that
     satisfied every step.
       - |C| == 1  -> resolved. Return it.
       - |C| == 0  -> the entity did not survive (deleted / merged away).
                      Run the Q3 NONE rules (fall back to parent/merge-result).
       - |C| >  1  -> the entity split, or the name is ambiguous (same
                      bounding-face pair yields several edges). Run the Q3
                      MANY rules (ordinal, object-space anchor, canonical key).
  4. Return (entity, confidence). confidence = Exact (single combinatorial
     match), Disambiguated (geometry/ordinal broke a tie), or Failed.
```

Step 2 is the heart: it is a **forward replay of CCH's "walk the operation order producing comparable names"** done as queries over Keel's live lineage map. Because every operation in Keel (boolean, chamfer, hole, shell) emits Modified/Generated/Deleted/SplitChild/MergeResult (file 07 mandate 2; dossier 47 for booleans), every Step is answerable from the kernel's own records. Nothing is re-derived geometrically in the common case; geometry enters only in Q3 tie-breaks.

**Confidence and the validity oracle.** Per Raghothama / Shapiro BR-variance (file 07): if the regeneration deformed the boundary continuously around the named entity (no SplitChild/MergeResult/Deleted touched its chain), confidence is Exact and the host trusts it silently. If a topology event touched the chain, confidence drops to Disambiguated or Failed, and the host surfaces it (FreeCAD's "detect, suggest, auto-repair only when confident", file 07). Auto-rebinding on low confidence re-introduces the bug.

### Sources for Q2

**OnShape / FeatureScript (queries as deferred resolution; tracking primitives).**
- Citation: OnShape FeatureScript Standard Library docs and developer forum (file 07 ref 19).
- Content (confirmed this pass): `evaluateQuery(context, query)` returns one transient query per matching entity **at the time of the call**, resolving against current geometry; `qUnion` preserves order so earlier subqueries list earlier; queries are state-based or historical; `startTracking` / `startTrackingIdentity` track entities (including the results of a split) through operations; `makeRobustQuery` keeps a reference valid when the feature itself modifies it (e.g. a boolean); transient queries do not track and go invalid on any change.
- Kernel relevance: validates the deferred-resolution model. Keel's `resolve(N, B')` is `evaluateQuery` against the regenerated body. `startTracking` for split results is exactly the Q3 split case: union the children so the reference resolves regardless of which child kept the transient id.

**Cascaval, Bodik, Schulz (2023), lineage-based referencing DSL.**
- Citation: *Proc. ACM PL* 7 (PLDI), 76-99 (file 07 ref 20). Language docs confirmed this pass.
- Content: operators `from`, `fromAny`, `fromAll`, `derivedFromAny`, `derivedFromAll`, plus `and`/`or`/`not`/`contains`; each CAD operation defines a lineage correspondence between input and output elements; references are predicates over durable lineage relations, so they re-resolve under parameter *and* program edits.
- Kernel relevance: the exact surface syntax for Keel's name-as-query. `derivedFromAll(f1, f2)` is precisely "the edge bounded by faces named f1 and f2" in Kripac structure. Confirms re-resolution is a query over lineage, not an index lookup.

**FreeCAD / realthunder / Ondsel MappedName (the concrete blueprint).**
- Citation: realthunder Topological Naming Algorithm; FreeCAD PRs #4752, #7427; Ondsel blog (file 07 refs 16-18).
- Content (confirmed): MappedNames encode operation history (`;:M` modified, `;:G` generated, op code such as `FUS`, tag/type metadata); multi-source merges nest sources in parentheses; split children get `;:G1`, `;:G2`; StringHasher (SHA1) compresses; `getElementHistory()` decodes back through the op chain.
- Kernel relevance: the proven serialization for a lineage path, and proof that the scheme works as a retrofit onto a kernel (OCCT) that exposes Modified/Generated/Deleted, which Keel also does. Keel's `Step` encoding and interning mirror MappedName and StringHasher one-to-one. The +30% recompute / +27% file-size overhead measured for FreeCAD is the budget to expect.

---

## Q3. Ambiguity from splits and merges

A name resolves to **several** (split, or a bounding-face pair that yields multiple edges) or to **none** (merge, deletion). Resolution rules, in priority order. The principle (dossier 47, Sugihara-Iri): decide combinatorially first, use geometry only to choose among combinatorially-valid candidates.

### When a name resolves to MANY (split, or inherently ambiguous edge)

1. **Ordinal from the SplitChild record (combinatorial, first choice).** When an operation splits a parent face into children, Keel's lineage emits `SplitChild#k` with a **stable ordinal** assigned by a deterministic rule (Q4 ties the boolean ordinal to imprint order). If the persisted name recorded the ordinal it wants, match it directly. CCH's "children inherit parent name plus ordinal" (file 07). If the host's reference was generic ("the face from this surface", no ordinal), return **all** children as the resolved set (the OnShape `startTracking`-the-split-and-union behavior, confirmed Q2): a generic reference to a face that split should mean "all of it", and downstream ops (a fillet on that face) apply to the whole. This is usually what the user meant and avoids silently picking one half.

2. **Object-space anchor (geometry, the tie-breaker).** If the reference wanted exactly one entity and the ordinal is unavailable or unstable, pick the candidate whose representative object-space point (Mun / Han OSI, stored at name time) is **closest** to the stored anchor point. Object-space, not parametric: it survives the reparametrization Keel does (file 07 Mun/Han vs Kripac). This is the single most useful disambiguator and the one OnShape exposes as `setExternalDisambiguation(context, id, entity)` (confirmed: "the identity of entity will be used to disambiguate the operation with that id"). The stored anchor IS Keel's external disambiguation token.

3. **Canonical ordering key (deterministic last resort).** If geometry still ties (two children equidistant, e.g. a symmetric split), order the candidates by a **canonical key** computed from build-independent data: lexicographic on `(surface type, anchor point rounded to tolerance, ordered bounding-face-name set)`. Pick the first. This is deterministic and reproducible (file 07 mandate 5), so the same edit always rebinds the same way, even if arbitrarily.

### When a name resolves to NONE (merge, deletion)

1. **Walk to the MergeResult (combinatorial, first choice).** If two faces merged into one (a separating feature was removed), Keel's lineage emits `MergeResult` linking the surviving face to **both** parents. The name of either parent re-resolves by following the parent->MergeResult edge to the survivor (FreeCAD nests merged sources, confirmed). So a NONE that is actually a merge becomes a single resolved entity (the merged face) at reduced confidence (Disambiguated). The host is told "your two references now name the same merged face."

2. **Fall back up the lineage to the nearest surviving ancestor.** If no MergeResult and no entity matches the full path, truncate the name to its longest prefix that still resolves, and return that ancestor with confidence Failed-but-suggested (FreeCAD `getRelatedElements` prefix-match recovery, file 07). The host surfaces it as "the exact entity is gone; did you mean its parent?" Never auto-bind.

3. **Genuine deletion -> ORPHANED.** If even the anchor's feature was suppressed/deleted, or no prefix resolves, report ORPHANED and stop. Cascaval's stated limit (file 07): lineage robustly identifies across edits but does not invent semantics when the lineage genuinely ceases to exist. Surfacing an honest break beats a confident wrong rebind.

### The disambiguators, summarized

- **Combinatorial first:** SplitChild ordinal, MergeResult edge. From the lineage, exact, build-independent.
- **Geometric tie-break:** object-space anchor point, nearest-wins. Survives reparametrization. = OnShape external disambiguation.
- **Deterministic last resort:** canonical ordering key. Reproducible, so rebinding is stable even when arbitrary.
- **Honest failure:** ancestor-prefix suggestion, then ORPHANED. Never a confident wrong bind.

### Sources for Q3

**Mun & Han (2005-2006), OSI + Secondary Name.**
- Citation: file 07 ref 12.
- Content: where a basic (combinatorial) name is not unique, disambiguate with Object Space Information (3D-model-space coordinates), more robust than Kripac's parametric (u,v) because object-space position survives reparametrization, plus a Secondary Name.
- Kernel relevance: the object-space anchor is Keel's primary geometric tie-breaker for MANY. Cheap to store, robust to Keel's canonical recovery.

**OnShape tracking / external disambiguation (split and ambiguity handling).**
- Citation: OnShape FeatureScript docs / forum (file 07 ref 19).
- Content (confirmed): `startTracking` tracks the results of a split; union the tracking queries so the reference resolves to the exact body whether or not its transient id changed. `setExternalDisambiguation(context, id, entity)` supplies an entity's identity to disambiguate an operation. `unstableIdComponent` marks an id component as a wildcard when it is not expected to be robust (e.g. an index into evaluated-query results).
- Kernel relevance: the split-then-union behavior is Keel's "generic reference returns all children." External disambiguation is the stored object-space anchor. `unstableIdComponent` warns Keel that ordinals into a query result are NOT durable unless minted from a stable rule (Q4).

---

## Q4. Matching after a boolean (where most renaming churn happens)

Booleans are where faces split, merge, and get reborn with new boundaries, so this is where naming churns most. The good news for Keel: **dossier 47 already mandates carrying identity through the boolean by `EdgeId`, and the same machinery carries names.** Persistent naming after a boolean is not new work; it is one more consumer of the imprint identity dossier 47 installs.

Tie to the pipeline (dossier 47): `preimprint -> seam_curves -> imprint_operand(a/b) -> classify_faces -> select_faces (KeptFace{operand,face,reversed}) -> assemble (identity-preserving radial splice, the recommended single path)`.

### Carrying names through imprint + classify + assemble

**1. Imprint is where split/merge name bookkeeping is minted.**
`imprint_operand` splits a source face along seam curves. Each resulting fragment is a **SplitChild of the source face**, and dossier 47 already requires the imprint to assign a stable `EdgeId` to every seam edge and store it on each fragment's coedges. Persistent naming adds one cheap obligation: **the imprint also emits a `SplitChild` lineage edge per fragment, with an ordinal derived from a build-independent rule.** The natural rule, free from the data dossier 47 already computes: order fragments by the **canonical key of the seam `EdgeId`s and the object-space anchor of each fragment** (sort by the lexicographically least seam `EdgeId` on the fragment, then by anchor point). Because `EdgeId`s and anchors are build-independent, the ordinal is stable across regeneration. The fragment **inherits the source face's name** (CCH) and appends `Boolean_k:SplitChild:source#ordinal`. A new seam edge is named by its **ordered bounding-face pair** (the two fragments it separates), Kripac/CCH structure, so it is named the instant it is created and needs no coordinate re-find.

**2. Classify + select carry the names untouched.**
`classify_faces` (generalized winding number, dossier 47) and `select_faces` decide which fragments survive and their orientation (`KeptFace.reversed`). They do **not** rename: a `KeptFace` keeps the SplitChild name its source fragment was given at imprint. Selection only prunes the name set (dropped fragments' names resolve to NONE -> deletion) and stamps orientation. Orientation is carried via `effective_sense = face.sense XOR reversed`, file-46 convention (dossier 47 Q6), and is part of the entity's identity for the radial splice, not part of its persistent name.

**3. Assemble inherits names by `EdgeId`, with split/merge bookkeeping.**
This is the payoff of dossier 47's recommendation to **retire `build_result_solid` and route the planar path through the identity-preserving `stitch_by_import`.** Because assembly groups coedges by `EdgeId` (not by coordinate) and asserts the closure invariant (every coedge has its radial partner, dossier 47 Q1), **names flow for free**:
- Each result face **inherits the name of the kept fragment it came from** (which inherited the source face's name via SplitChild at imprint). A box face that survives the boolean keeps `Box1...top / Boolean1:Modified:top`. The `Modified` (not `Generated`) kind records that it is the same surface with a re-cut boundary.
- Each result edge is named by its **ordered bounding-face-name pair**, read directly off the two faces meeting at its `EdgeId`. Since assembly already knows, per `EdgeId`, exactly which kept fragments meet there (that is what the radial group IS), the edge's bounding-face-pair name is available with zero extra geometry. **This is the precise link to dossier 47's `EdgeId` identity:** the same `EdgeId` group that proves shell closure also yields the edge's persistent name.
- **Split bookkeeping:** a source face that imprint cut into several kept fragments produces several result faces, each a SplitChild with its stable ordinal. A reference to the original face resolves to MANY -> all children (Q3 rule 1).
- **Merge bookkeeping:** when the boolean makes two coplanar same-surface fragments from different operands into one result face (a same-domain merge, dossier 47's `myShapesSD` analogue in OCCT BOPAlgo), assembly emits a `MergeResult` linking the result face to both source-face names. A reference to either source resolves via the merge edge to the single result (Q3 NONE rule 1).

### Why this is robust where the soup path was not

Dossier 47 showed `build_result_solid` drops the tilted cut face because it re-welds by coordinate. The **same defect would corrupt naming**: a face dropped by the assembler has no name to inherit, and a face whose edges were re-welded loses the `EdgeId` that its bounding-face-pair name is built from. **Persistent naming and correct assembly are the same requirement viewed twice.** Adopting dossier 47's identity-preserving radial-splice assembly is a precondition for boolean name-carrying: you cannot name-by-`EdgeId` if assembly threw the `EdgeId` away. This is the strongest internal cross-check in this dossier: the boolean-result-assembly fix (47) and the persistent-naming feature (52) are mutually reinforcing and must ship together.

### Sources for Q4

**Dossier 47 (Keel boolean result assembly).**
- Citation: `docs/research/kernel/47-boolean-result-assembly.md`.
- Content: imprint assigns stable `EdgeId`; fragments store the `EdgeId` of the edge they lie on; assembly groups coedges by `EdgeId` (not coordinate), splices the radial cycle, asserts closure; retire `build_result_solid` for the identity-preserving `stitch_by_import`; orientation carried via `effective_sense = sense XOR reversed` (file-46).
- Kernel relevance: the exact substrate Q4 rides on. SplitChild ordinals, bounding-face-pair edge names, and MergeResult bookkeeping are all read from the `EdgeId` groups assembly already builds. No new geometry.

**Requicha & Voelcker (1985), boundary evaluation and merging.**
- Citation: file 47 ref 1.
- Content: subdivision (= imprint) creates shared edges; classification; merging connects over the shared edges, never re-derives them.
- Kernel relevance: the subdivision stage is where Keel mints SplitChild names; merging is where faces inherit them. Names follow the same three stages as the boolean itself.

**Weiler (1988), radial edge structure.**
- Citation: file 47 ref 3.
- Content: a non-manifold edge is its radial cycle of incident faces; oriented edge-uses (coedges) vs the abstract edge.
- Kernel relevance: an edge with >2 incident faces is named by its **ordered bounding-face set** (generalizing the pair), read off the radial cycle. Keel's non-manifold naming is structurally possible because the PES records the cycle.

---

## Persistent naming for Keel: synthesis

**Chosen approach.** A **Capoyleas / Chen / Hoffmann generative-history naming scheme**, structured **Kripac-style** (faces primary; edges/vertices named by their ordered bounding-face set), carried as **Keel lineage edges**, queried in **Cascaval lineage-DSL** form (`from`, `derivedFromAny`, `derivedFromAll`, set ops), disambiguated by **Mun / Han object-space anchors** and a deterministic canonical key. NOT Bidarra / Bronsvoort SFM as the kernel mechanism (that is a host-layer option Keel's non-manifold substrate enables). The decisive reason: CCH's "name = operation + operands + derivation kind" **is already Keel's lineage map**; this is a promote-and-replay feature, not new kernel machinery.

**The lineage-based name structure.** A persisted name is a **lineage path** rooted at host-durable feature-input anchors (sketch profiles, primitive ops, datums, whose ids the host re-supplies every regeneration), then a sequence of `op_id : derivation_kind : operand_selector [#ordinal] [@anchor]` steps, terminating at a surface-bearing step (faces) or an ordered bounding-face-name pair (edges). Steps are **interned/hashed** (FreeCAD StringHasher) because names compound through the feature tree. One **object-space anchor point** is stored per named entity for tie-breaking.

**Re-resolution.** `resolve(N, B')`: bind the root anchors to the regenerated body's current inputs by host-stable feature id; **forward-replay** each lineage step as a query over `B'`'s live lineage map (matching derivation events, never coordinates); collect candidates; return Exact (one combinatorial match), Disambiguated (geometry/ordinal broke a tie), or Failed. Confidence rides on a Raghothama / Shapiro BR-variance check: continuous deformation -> trust silently; a topology event on the chain -> surface to the host (detect, suggest, auto-repair only when confident).

**Split/merge disambiguation.** MANY: SplitChild ordinal (combinatorial, build-independent) -> generic references return ALL children (OnShape track-and-union) -> object-space anchor nearest-wins (= OnShape external disambiguation) -> canonical key (deterministic last resort). NONE: walk to MergeResult -> longest-resolving ancestor prefix as a suggestion (FreeCAD getRelatedElements) -> ORPHANED (never a confident wrong bind; Cascaval's honest limit).

**Boolean name-carrying.** Imprint mints SplitChild names with ordinals from a build-independent rule (least seam `EdgeId` + anchor) and names new seam edges by bounding-face pair; classify/select carry names untouched and stamp orientation; **assemble inherits names by `EdgeId`** off the same radial groups that prove shell closure (dossier 47), with split (-> several SplitChildren) and merge (-> MergeResult linking both sources) bookkeeping. **Persistent naming requires dossier 47's identity-preserving assembly; they ship together.**

### Validation: box -> chamfer -> hole edit chain

Goal: a host reference to **a box face** must survive a chamfer and then a hole regeneration.

1. **Build box.** Top face named `Box1.primitive:Created:top`, anchor at its centroid. Host stores this name for "the face I want to drill later."
2. **Chamfer an edge of the box.** The chamfer modifies the top face's boundary (one corner beveled) but not its surface. Regenerate. `resolve(Box1.primitive:Created:top, B')`: anchor `Box1` binds; forward-replay finds the top face now carrying `... / Chamfer1:Modified:top` in `B'`'s lineage; `|C| == 1`; **Exact**. The chamfer also generated a new bevel face `Chamfer1:Generated:bevel#0` (named, but not the reference of interest). BR-variance: the top face deformed continuously (Modified, not Split), so confidence is Exact and the host is not bothered. **Reference survives.**
3. **Drill a hole through the top face.** The hole is a boolean (or local op) that imprints a circular seam into the top face, splitting it into the face-with-a-hole. If Keel models this as the top face Modified (boundary gained an inner loop, same surface), `resolve` still returns one Exact match: `... / Chamfer1:Modified:top / Hole1:Modified:top`. If instead the hole splits the top face into fragments, the reference resolves to MANY -> all children (Q3 rule 1) -> the host gets the whole top region, which is the intended "the top face, now with a hole." Either way **the original reference survives the hole regeneration.** The hole also generates a cylindrical wall `Hole1:Generated:wall` and a bottom edge named by its bounding-face pair (wall, bottom-or-through), all freshly and stably named.
4. **Edit the chamfer setback and regenerate the whole chain.** Anchors are unchanged; every step replays; the top-face reference re-resolves Exact because its lineage chain (Created -> Modified -> Modified) is intact even though every `EntityId` in `B'` is different from the first build. This is the item-82-85 guarantee: **the reference is to the derivation, not to the id.**

If, conversely, the host had stored the raw `EntityId` `Face#7`, step 2 alone would rebind it to whatever `Face#7` happens to be after the chamfer renumbers, the classic FreeCAD bug. The lineage path is what makes it survive.

**This is a design recommendation. Validate against Keel's tests:** add a persistent-naming regression that runs the box -> chamfer -> hole chain, stores the top-face name after step 1, and asserts it resolves Exact (or MANY-all-children) after steps 2, 3, and 4 with all `EntityId`s renumbered; assert a deleted-feature case reports ORPHANED rather than rebinding; assert a merge case (remove a separating feature) resolves via MergeResult. Run it alongside dossier 47's asymmetric-chamfer assembly repro, since boolean name-carrying depends on the identity-preserving assembly fix.

---

## References

1. J. Kripac. "A mechanism for persistently naming topological entities in history-based parametric solid models." *Computer-Aided Design* 29(2), 113-122, 1997. https://www.sciencedirect.com/science/article/abs/pii/S0010448596000401 ; Solid Modeling '95: https://www.semanticscholar.org/paper/A-mechanism-for-persistently-naming-topological-in-Kripac/a5e40e5ac256c3d3c9beb7678ab9ffe6ac3d1e6f
2. V. Capoyleas, X. Chen, C. M. Hoffmann. "Generic naming in generative, constraint-based design." *Computer-Aided Design* 28(1), 17-26, 1996. https://www.cs.purdue.edu/cgvlab/www/resources/papers/Capoyleas-Computer_aided_design-1996-Generic_naming_in_generative.pdf
3. R. Bidarra, P. J. Nyirenda, W. F. Bronsvoort. "A feature-based solution to the persistent naming problem." *Computer-Aided Design and Applications* 2(1-4), 517-526, 2005. https://graphics.tudelft.nl/~rafa/myPapers/jrnl-bidarra.CAD05.pdf
4. D. Mun, S. Han. "Identification of topological entities and naming mapping for parametric CAD model exchanges." *Int. J. CAD/CAM*, 2005-2006. https://koasas.kaist.ac.kr/bitstream/10203/6600/1/Identification%20of%20Topological%20Entities%20and%20Naming.pdf
5. S. H. Farjana, S. Han. "Mechanisms of persistent identification of topological entities in CAD systems: A review." *Alexandria Engineering Journal* 57(4), 2837-2849, 2018. https://www.sciencedirect.com/science/article/pii/S1110016818300814
6. S. Raghothama, V. Shapiro. "Boundary representation deformation in parametric solid modeling." *ACM TOG* 17(4), 259-286, 1998. https://dl.acm.org/doi/10.1145/293145.293148
7. realthunder. "Topological Naming Algorithm." FreeCAD_assembly3 / asm3-wiki. https://github.com/realthunder/asm3-wiki/blob/master/Topological-Naming-Algorithm.md
8. FreeCAD documentation. "Topological naming problem." https://github.com/FreeCAD/FreeCAD-documentation/blob/main/wiki/Topological_naming_problem.md ; Ondsel toponaming milestone: https://www.ondsel.com/blog/milestone-toponaming-fix-phase-2-done/
9. FreeCAD/FreeCAD DeepWiki. "TopoShape and Topological Naming" (MappedName, StringHasher, getElementHistory). https://deepwiki.com/FreeCAD/FreeCAD/2.1-toposhape-and-topological-naming
10. OnShape. FeatureScript Standard Library Documentation (evaluateQuery, qUnion, qCreatedBy, startTracking, makeRobustQuery, setExternalDisambiguation, unstableIdComponent). https://cad.onshape.com/FsDoc/library.html
11. OnShape forum. "How does the identity tracking/robustness system work?" https://forum.onshape.com/discussion/16911/how-does-the-identity-tracking-rebustness-system-work ; "Difference Between Make Robust Query and Set External Disambiguation": https://forum.onshape.com/discussion/14827/difference-between-make-robust-query-and-set-external-disambiguation
12. D. Cascaval, R. Bodik, A. Schulz. "A Lineage-Based Referencing DSL for Computer-Aided Design." *Proc. ACM PL* 7 (PLDI), 76-99, 2023. https://dcascaval.github.io/lineage-based-cad-referencing-pldi23.pdf ; operators: https://github.com/dcascaval/lineage-based-cad-referencing/blob/main/language-docs.md
13. A. A. G. Requicha, H. B. Voelcker. "Boolean operations in solid modeling: boundary evaluation and merging algorithms." *Proc. IEEE* 73(1), 1985. https://ieeexplore.ieee.org/abstract/document/1457376/
14. K. Weiler. "The Radial Edge Structure: A Topological Representation for Non-Manifold Geometric Boundary Modeling." Geometric Modeling for CAD Applications, 1988.

### Related Keel dossiers (build on, not repeated)
- `docs/research/kernel/07-persistent-naming-sessions.md` (the literature survey this file does not repeat: Kripac, CCH, Marcheix, Bidarra/Bronsvoort, Mun/Han, Wang, Raghothama/Shapiro, FreeCAD/realthunder, OnShape, Cascaval; plus the 14 naming/session mandates for Keel).
- `docs/research/kernel/47-boolean-result-assembly.md` (EdgeId identity from imprint, the closure invariant, KeptFace, identity-preserving radial-splice assembly: the substrate for Q4 boolean name-carrying).
- `docs/research/kernel/46-face-orientation-sense-convention.md` (n_out = sense x natural; orientation carried, not re-derived).
- `docs/research/kernel/19-parametric-regeneration.md` (regeneration / replay context for the host feature tree).
- `docs/research/kernel/14-determinism-serialization.md` (deterministic re-evaluation, the precondition for any naming scheme).
