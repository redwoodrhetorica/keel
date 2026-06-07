# 13. Import Healing Pipelines and Surface Extension

Research dossier supporting the design of **Keel**, an open-source B-rep solid modeling kernel in Rust with Parasolid-class ambition.

## Scope and motivation

Import quality is where kernels earn adoption. A kernel that only models its own native geometry is a toy. The real world arrives as STEP, IGES, JT, native Parasolid/ACIS, and faceted dumps, each carrying the accumulated rounding, tolerance philosophy, and bugs of the system that wrote it. Translated bodies show up with vertex gaps, edge gaps, face gaps, mismatched tolerances, sliver faces, degenerate faces, missing or wrong pcurves, self-intersecting trim loops, and inconsistent face orientations. None of these are boolean-ready. A healing pipeline turns that soup into watertight, orientable, tolerance-coherent solids.

Surface extension is the small load-bearing companion to healing. Delete-face healing, tweak re-intersection, and untrimming all require pushing a B-spline (or analytic) surface beyond its current parametric domain so neighbors can be re-intersected to rebuild the wound. Extension is deceptively dangerous: polynomial extrapolation degrades fast, rational weights can go negative, and a naive extension can fold the surface. This file treats both subjects in depth.

This dossier is organized as:

- **Part 1: Import healing** (problem taxonomy, stitching/sewing, commercial pipelines, geometry repair, tolerant-vs-exact, validation/PDQ, academic and learning-based repair)
- **Part 2: Surface extension** (extension constructions, extrapolation stability, healing/local-ops context, untrimming)
- **Healing pipeline architecture for Keel** (synthesis: phase ordering, heal-vs-tolerate policy, extension service requirements)
- **References**

Each significant source gets a Citation / Method / Limitations / Kernel relevance entry.

---

# PART 1: Import healing

## 1.1 The problem taxonomy and CAD model quality surveys

### Source: Gonzalez-Lluch et al., survey on CAD model quality assurance

**Citation.** Gonzalez-Lluch, C., Company, P., Contero, M., Camba, J. D., & Plumed, R. (2017). A survey on 3D CAD model quality assurance and testing tools. *Computer-Aided Design, 83*, 64-79. https://doi.org/10.1016/j.cad.2016.10.003 (preprint arXiv:1611.01765).

**Method.** The paper builds a hierarchical taxonomy of CAD model quality that is the most useful single organizing frame for a healing pipeline's diagnosis stage. Two axes structure it. First axis: representation type, **explicit** models (the dumb-solid B-rep that arrives from translation, carrying no construction history) versus **procedural** models (feature trees, where the editor can prevent some errors at authoring time). Second axis: three error levels. **Morphologic** errors are geometric/topological correctness failures (gaps, degenerate faces, self-intersections, bad orientation, slivers). **Syntactic** errors are structure/format violations (broken references, non-conforming entities). **Semantic** errors concern reusability and design intent (over-complex geometry, missing parametric structure). The authors then classify existing commercial and academic quality-testing tools against this taxonomy, showing that low-semantic morphologic errors that hamper simplification are reasonably well covered in explicit representations, while higher semantic quality is barely supported.

**Limitations.** It is a survey, not an algorithm. It catalogs what tools detect, not how to fix. Semantic-level quality is left as an open problem. The taxonomy is descriptive and does not prescribe phase ordering.

**Kernel relevance.** Adopt the morphologic level wholesale as Keel's diagnostic checklist: this is precisely the set of defects a healer must detect and repair. The explicit-vs-procedural split tells Keel where it lives: imported bodies are explicit, so the healer cannot lean on construction history and must work geometry-first. Use the taxonomy to structure the `HealReport` that diagnosis emits.

### Source: morphologic defect classes (synthesized across the literature)

The recurring morphologic taxonomy across the survey literature, the ACIS/Parasolid/CADfix documentation, and the PDQ standards converges on these classes, which Keel should model as first-class diagnostic codes:

- **Gaps by dimension.** *Vertex gaps* (two vertices that should coincide are separated by more than vertex tolerance). *Edge gaps* (two edges that should be shared are separate curves with a sliver of space between them; the body is a face soup, not a shell). *Face gaps / missing faces* (a region of the intended solid has no face at all, leaving an open shell).
- **Tolerance mismatch between systems.** Every source CAD system carries its own internal linear resolution (e.g. ~1e-8 in some, larger in others) and its own angular tolerance. A model healed to one system's notion of "coincident" looks gappy to a tighter kernel. Keel's import must record the source's stated tolerance and not assume its own.
- **Degenerate and sliver faces.** Zero-area faces, faces collapsed to a line, knife-edge slivers from near-tangent trims, faces narrower than the kernel's resolution.
- **Bad or missing pcurves.** A trimmed face needs, for each edge, a 2D curve in the surface's (u,v) domain. Translators frequently drop pcurves, supply pcurves that do not project onto the 3D edge, or supply pcurves shifted by a period on a closed surface.
- **Self-intersecting trim loops.** A face's boundary wire crosses itself in parameter space, so the face has no well-defined interior.
- **Inconsistent orientations.** Adjacent faces disagree on which side is solid, so the shell is not coherently orientable.

These classes drive every downstream pipeline decision and recur in all the commercial documentation below.

---

## 1.2 Stitching and sewing algorithms

### Source: Barequet & Sharir, filling gaps in the boundary of a polyhedron

**Citation.** Barequet, G., & Sharir, M. (1995). Filling gaps in the boundary of a polyhedron. *Computer Aided Geometric Design, 12*(2), 207-229. https://doi.org/10.1016/0167-8396(94)00011-G.

**Method.** The seminal geometry-based stitching paper. Input is a polyhedral boundary corrupted by small gaps, defined as gaps bounded by edges incident to only one face (boundary edges of a not-quite-closed surface). The algorithm has two engines. (1) **Partial curve matching**: it matches pieces of the gap boundary to each other using a curve-matching technique that scores how well two boundary chains correspond, allowing partial (sub-chain) matches. (2) **Optimal triangulation of 3D polygons**: whatever the matching leaves unresolved is filled by computing a minimum-cost triangulation of the residual 3D polygonal hole. The authors prove that finding a consistent set of partial matches with maximum total score is NP-hard, so the practical algorithm is a heuristic over the matching graph followed by hole triangulation.

**Limitations.** Operates on faceted/polyhedral boundaries, not on the exact NURBS B-rep that Keel must heal. The NP-hardness of optimal matching forces heuristics on large gap sets. It fills gaps with triangles, which is a meshing answer, not a smooth-surface answer.

**Kernel relevance.** The two-engine decomposition is the right mental model even for exact B-rep: **match what you can pair up (edge-to-edge stitching), then synthesize geometry for what is left over (gap fill / missing-face construction).** Keel's stitcher should first attempt edge pairing under tolerance, then escalate residual unmatched boundary chains to a fill step. The NP-hardness result is a warning: do not seek global optimality in matching; use greedy nearest-match-under-tolerance and accept it.

### Source: OpenCASCADE BRepBuilderAPI_Sewing and Shape Healing

**Citation.** Open CASCADE Technology. (2024). *Shape Healing user guide* and *BRepBuilderAPI_Sewing* reference. Open Cascade SAS. https://dev.opencascade.org/doc/overview/html/occt_user_guides__shape_healing.html

**Method.** OCCT is the most thoroughly documented open-source healer and its phase structure is directly portable. Sewing (`BRepBuilderAPI_Sewing`) takes a face soup and a working tolerance, finds edges that are geometrically coincident within tolerance, and merges coincident edge groups into single shared edges and coincident vertex groups into single vertices, producing a connected shell. Advanced sewing additionally calls into Shape Healing (`ShapeFix_Shape`) for tricky cases such as closed surfaces. Shape Healing itself is a strict hierarchy operating finest-to-coarsest: **Solid -> Shell -> Face -> Wire -> Edge**. `ShapeFix_Wire::Perform()` runs a fixed-order sequence that is worth copying verbatim: (1) FixReorder (put edges in connected sequence), (2) FixSmall (drop edges shorter than tolerance), (3) FixConnected (force adjacent edges to share a vertex), (4) FixEdgeCurves (reconcile 3D curve with 2D pcurve, including FixReversed2d, FixRemovePCurve, FixAddPCurve, FixShifted for period-shifted seam pcurves), (5) FixDegenerated (insert degenerate edges at poles), (6) FixSelfIntersection (resolve 2D self-crossings), (7) FixLacking (fill parametric gaps between consecutive pcurves). `ShapeFix_Face` handles wire orientation on the face, inserts missing seam edges on periodic surfaces, and removes tiny-area wires. `ShapeFix_Shell` makes face orientations coherent (and detects non-orientable Mobius cases). `ShapeFix_Edge` adds missing 3D curves or pcurves and fixes the SameParameter flag (3D and 2D representations must agree within edge tolerance). Tolerance escalation is pervasive and capped by `SetMaxTolerance()`: when geometry modification is forbidden, fixes for non-connected and self-intersecting wires proceed by *raising the local tolerance* until the defect is absorbed, failing only if the cap is exceeded. `ShapeUpgrade` is the orthogonal toolkit: ShapeDivideContinuity splits surfaces at C0 lines to reach a target continuity, ShapeDivideArea splits oversized faces, ShapeConvertToBezier decomposes into Bezier patches.

**Limitations.** The fixed Perform() order is rigid; some defect combinations need iteration that OCCT does not always perform automatically. Tolerance escalation hides defects rather than removing them, inflating downstream tolerance. OCCT's pcurve projection (FixAddPCurve) can fail on poorly-conditioned surfaces. The hierarchy can produce valid-but-ugly results (heavily tolerant edges) that are hard to boolean later.

**Kernel relevance.** This is the single most directly reusable design. Keel should adopt the finest-to-coarsest hierarchy and the explicit ordered Wire fix list almost unchanged. The two-mode design (ModifyTopology vs ModifyGeometry) maps cleanly onto Keel's heal-vs-tolerate policy. The MaxTolerance cap is the key safety valve. Treat OCCT's Perform() order as the reference phase ordering for wire-level healing.

---

## 1.3 Commercial healing pipelines as documented

### Source: 3D ACIS Modeler HEAL component

**Citation.** Spatial Corp. (Dassault Systemes). *3D ACIS Modeler Healing Component documentation* (HEAL, R10 and later); see also Spatial Corp. (2021), *How the 3D ACIS Modeler improves the quality of translated 3D data.* https://blog.spatial.com/acis-modeler-3d-translated-data

**Method.** ACIS HEAL is the canonical three-phase healing architecture and the one whose phase names entered the industry vocabulary. **Phase 1, stitching**: replace groups of coincident edges with a single edge and groups of coincident vertices with a single vertex, joining separate faces into a topologically complete sheet or solid body. **Phase 2, geometry simplification (analytic conversion)**: imported models often carry surfaces that look analytic on screen but are stored as splines; this phase recognizes and converts spline surfaces back to their exact analytic forms (plane, cylinder, cone, sphere, torus) wherever the spline lies within a tolerance (default ~1e-4 in model units) of an analytic. This shrinks data, speeds later operations, and makes intersections exact. **Phase 3, gap tightening (geometry building, "geombuild")**: tighten inaccuracies by recomputing geometry. Internally geombuild runs a sequence of specialized solvers, each attacking a class of edges/vertices: an analytic solver for analytic-analytic junctions (using a graph-based solver that handles cyclic tangencies, where surface nodes and tangent-edge arcs form a graph and higher-curvature surfaces are preferred when resolving), an isospline solver (corrects spline surfaces by control-point modification when the intersection curve aligns with an isoparametric line), a sharp-edge solver, and a generic spline solver, followed by a wrap-up. Results and per-phase diagnostics are tracked in result structures (e.g. bhl_geometry_results, hh_geombuild_options).

**Limitations.** Analytic conversion is lossy and tolerance-dependent: too tight and nothing converts, too loose and shape drifts. Geombuild's solver cascade is order-sensitive and can leave residual gaps when the analytic and spline solvers disagree at a junction. The default tolerances are tuned for typical automotive/mechanical data and may misbehave on micro or macro scale parts.

**Kernel relevance.** Adopt the three-phase skeleton: **stitch, then simplify (analytic recognition), then build geometry (gap tightening by re-intersection).** The ordering matters: simplify before building, because exact analytic surfaces re-intersect cleanly whereas splines do not. The graph-based tangency solver and the isospline control-point correction are concrete algorithms Keel can implement. Per-phase result tracking is the model for Keel's `HealReport`.

### Source: CADfix (ITI / Wipro) diagnosis-and-repair pipeline

**Citation.** International TechneGroup Inc. (ITI, a Wipro company). *CADfix and CADfix DX* product documentation and technical material. https://www.iti-global.com/interoperability-products/cadfix/ ; CAD Interop, *Process automation with CADfix repair.* https://www.cadinterop.com/en/your-needs/cad-repair-and-healing/process-automation-with-repair.html

**Method.** CADfix documents the most explicit *interactive-then-automated* workflow. Stages: **Smart import** (read with maximum information preservation). **Automatic analysis/diagnosis** (full scan, color-coding edges green for shared/connected and red for open/free, classifying defects by category and severity: stitching gaps small and large, missing faces, degenerate NURBS, self-intersecting surfaces, fragmented loops, plus non-recoverable defects flagged for human review). **Visualization** (graphic presentation of defects). **Guided repair** with three escalating methodologies: automatic (close 4-edge loops, stitch via *progressively widening tolerance until a solid is rebuilt*), guided (operator validates each fix), expert (surface blending/reconstruction for complex geometry). **NURBS simplification** (convert unstable NURBS to plane/cylinder/sphere/torus where tolerance permits). **Defeaturing** (remove holes, fillets, lettering, small entities, with handling of conjoined/overlapping fillets). **Dynamic validation** (real-time re-diagnosis). **Prepare/output** (retarget tolerances and surface types to the destination system). Once an interactive sequence works, it is frozen into a .cfg and replayed via CLI, watched folders, or a server/REST API for batch automation.

**Limitations.** The richest capabilities are interactive; full automation works for "typical" data but escalates hard cases to humans. Adaptive tolerance widening (like OCCT) can mask rather than fix. The published material is product marketing, so internal algorithms are described at a high level only.

**Kernel relevance.** Two ideas transfer directly. (1) **Adaptive tolerance escalation as a controlled loop**: attempt stitch at the source tolerance, widen in steps, stop at a hard cap, and *record* every widening so callers know how much the model was perturbed. (2) **The diagnose-record-replay model**: Keel should be able to emit a healing recipe (an ordered list of operations with parameters) that can be re-applied in batch. The green/red free-edge visualization maps onto a Keel API that returns the set of non-manifold/free edges.

### Source: Theorem and Elysium translation/healing approaches

**Citation.** Theorem Solutions, *CADverter / Unified Interface*; Elysium, *ASFALIS / CADdoctor* product documentation (industry interoperability literature; see CAD Interop and SASIG quality-control material referenced below).

**Method.** Both vendors implement direct-translation plus healing pipelines comparable in shape to ACIS HEAL and CADfix. The distinctive emphasis in the documented material is **PDQ-driven healing**: rather than healing to a generic notion of validity, they heal to satisfy a named quality standard (SASIG/VDA/JAMA criteria) and produce a pass/fail PDQ report. CADdoctor (Elysium) is documented as performing automated healing followed by quality verification against configurable rule sets, and as supporting simplification/defeaturing for downstream CAE.

**Limitations.** Documentation is product-level; algorithmic detail is sparse. The standards-conformance framing is automotive-centric (VDA/JAMA), which may not match Keel's broader audience.

**Kernel relevance.** Reinforces that *healing and validation are two halves of one loop*: heal, then check against an explicit criterion set, then re-heal residuals. Keel should pair its healer with a PDQ-style checker (Section 1.6) and let the target standard parameterize how aggressively to heal.

---

## 1.4 Geometry repair: pcurves, refitting, simplification

### Source: pcurve regeneration and SameParameter (OCCT FixAddPCurve / FixSameParameter, generalized)

**Method.** The most common geometry repair is pcurve regeneration. When an edge lacks a valid 2D curve, the healer projects the 3D edge curve onto the supporting surface's (u,v) domain to synthesize a pcurve, then forces endpoint agreement by adjusting vertex tolerance. The dual problem is the SameParameter condition: the 3D curve C(t) and the pcurve P(t) lifted through the surface S(P(t)) must agree within edge tolerance at equal parameter t. When they disagree, the healer either reparameterizes the pcurve, refits it, or (last resort) raises edge tolerance to cover the maximum deviation. On closed/periodic surfaces a regenerated pcurve can land a full period off (FixShifted), so the projector must choose the period branch nearest the adjacent edges. Robust projection is the hard part: it must handle surfaces with poles, near-degenerate parameterization, and self-overlapping domains.

**Limitations.** Projection is ill-conditioned where the surface is nearly degenerate or has high parametric distortion; the projected pcurve can wander or oscillate. Forcing SameParameter by tolerance inflation degrades downstream boolean precision.

**Kernel relevance.** Keel needs a robust, well-tested **3D-edge-to-surface projector** as a core service, used both in healing (pcurve regen) and in modeling. Treat SameParameter as an invariant the kernel maintains and verifies; expose a "regenerate pcurves" healing operation. Prefer refit-pcurve over raise-tolerance, matching the heal-vs-tolerate policy below.

### Source: entity simplification / analytic surface recognition

**Citation.** Synthesized from ACIS HEAL geometry-simplification phase and CADfix NURBS simplification (cited above), plus the classical surface-fitting literature.

**Method.** Translated splines that approximate analytics are recognized and replaced by exact plane, cylinder, cone, sphere, or torus. The recognition test samples the spline, fits a candidate analytic by least squares (or by examining the surface's curvature signature: zero Gaussian and zero mean curvature implies plane; constant nonzero principal curvature with one zero implies cylinder; etc.), and accepts the analytic if the maximum deviation over the patch is within tolerance. The benefits are threefold: smaller data, faster and more robust intersections (analytic-analytic intersections are closed-form), and cleaner downstream booleans. The same machinery merges tiny edges and faces (collapse a sliver face into its neighbor, merge two near-collinear edges into one).

**Limitations.** Recognition is tolerance-gated and can mis-recognize (forcing a near-cylinder to an exact cylinder shifts geometry). Merging tiny features changes topology and can break persistent naming. Curvature-signature tests are sensitive to the spline's parameterization quality.

**Kernel relevance.** High value. An analytic-recognition pass markedly improves the booleanability of imported data and aligns with Keel's likely preference for exact analytic surfaces internally. Implement it as a post-stitch, pre-geombuild phase (matching ACIS ordering). Gate every conversion on a verified deviation bound, and emit it into the heal report so naming can adapt.

---

## 1.5 Tolerant modeling: the alternative to exact healing

### Source: Parasolid tolerant modeling and body healing

**Citation.** Siemens Digital Industries Software. *Parasolid functional description: tolerant modeling, body healing, and checking* (Parasolid V35 documentation, 2022); see also the PK body-healing and tolerant-edge/vertex material. http://www.q-solid.com/Parasolid_Docs_V35/

**Method.** Parasolid's answer to imperfect data is to *represent* the imperfection rather than always remove it. Each boundary entity (face, edge, vertex) carries its own **geometric tolerance**. Geometrically, an edge is a tube and a vertex is a sphere; lowering local precision thickens the tube and grows the sphere. An edge whose 3D curve does not lie exactly on both adjacent faces is made a **tolerant edge** whose tolerance covers the maximum gap; two faces that do not intersect cleanly are joined by a tolerant edge instead of an exact intersection curve. Vertices where edges fail to meet exactly become **tolerant vertices**. The boolean and coincidence machinery is written to honor these per-entity tolerances, so a body that could never be made exact can still be intersected and unioned. Body healing is the complementary path: PK body-healing/fault-fixing detects self-intersections, degeneracies, and overlapping tolerances and attempts to remove them, and Parasolid can *replace tolerant edges with accurate (exact) edges* when the geometry permits (controlled in translators by an accurate-edge option). Short edges can be collapsed into tolerant vertices. To use tolerant modeling deliberately, one lowers local precision until the geometric data intersects; entities without a set local precision are treated as exact.

**Limitations.** Tolerant entities are contagious: every boolean that touches a tolerant edge tends to spread tolerance into the result, slowly degrading precision. Tolerant geometry is slower and complicates downstream operations (filleting, offsetting). It is a representation of failure, not a fix; the model is still geometrically imperfect.

**Kernel relevance.** This is the central architectural decision for Keel. The choice is not heal *or* tolerate; it is a **policy**: heal first, tolerate only what cannot be healed within the tolerance cap, and prefer to re-tighten tolerant entities to exact when later geometry allows. Keel must decide early whether to adopt per-entity tolerances (the Parasolid model) as a first-class part of its B-rep. The recommendation in the synthesis below is yes: a tolerance-aware B-rep is what lets imported data survive at all, and exact-only kernels are brittle on real imports.

---

## 1.6 Validation and PDQ standards

### Source: SASIG PDQ / VDA 4955 / ISO PAS 26183

**Citation.** SASIG (Strategic Automotive product data Standards Industry Group). *Product Data Quality Guidelines for the Global Automotive Industry*, V2.1, published as ISO/PAS 26183:2006; derived from VDA 4955. International Organization for Standardization.

**Method.** PDQ standards define, for each quality criterion, *what the defect is, how to measure it, and what to do about it.* They focus on the **morphologic** quality level: geometric validity (degenerate faces, open/free edges, invalid intersections, slivers), mathematical precision (tolerance and continuity conformance), topological consistency (correct entity relationships), and structural conformity (assembly organization), with a thin layer of semantic criteria (attributes/metadata). Each criterion has a measurable threshold so that conformance is a pass/fail (or graded) test, enabling automated checkers.

**Limitations.** Automotive-centric and consensus-driven, so thresholds reflect that industry's tolerances. Covers detection and measurement, not repair algorithms. Semantic quality is acknowledged but barely specified.

**Kernel relevance.** Use PDQ as the **specification for Keel's post-heal validator**. The "measure each criterion against a threshold" structure is exactly how `body_check`/`heal_verify` should report. Let the active PDQ profile parameterize healing aggressiveness.

### Source: ISO 10303-59, quality of product shape data

**Citation.** ISO. (2022). *ISO 10303-59: Industrial automation systems and integration. Product data representation and exchange. Part 59: Integrated generic resource: Quality of product shape data.* International Organization for Standardization. (Earlier edition 2014.)

**Method.** The STEP integrated resource that formalizes shape-data-quality representation so quality requirements, declarations, and assurances can travel *with* the model in a STEP file. It provides EXPRESS schema for expressing quality criteria and measured values on shape entities, complementing the PDQ guidelines (which say what to measure) with a standardized way to record results.

**Limitations.** A representation standard, not an algorithm; defines how to store quality data, not how to compute or fix it. Adoption in practice is partial.

**Kernel relevance.** If Keel writes STEP, it can attach AP242/Part-59 quality declarations describing what it healed and the residual tolerances, giving downstream consumers provenance. Lower priority than the healer itself but valuable for a kernel that markets trustworthy import.

### Source: PDQ representation for shape data (JCISE)

**Citation.** Yang, J., Han, S., & Kang, T. W. (2010). PDQ (Product Data Quality): Representation of data quality for product data and specifically for shape data. *Journal of Computing and Information Science in Engineering, 10*(2), 021003. https://doi.org/10.1115/1.3402642

**Method.** Proposes a formal model for representing product-data-quality information, with specific treatment of shape data, that underlies and connects the SASIG and ISO 10303-59 efforts. It distinguishes the quality *criteria*, the *measurement* of a criterion on a model, and the *requirement/declaration* of quality, giving a data model that a checker and a translator can both target.

**Limitations.** Conceptual/representational; light on healing algorithms. Validation in industry tooling is partial.

**Kernel relevance.** Gives Keel a clean separation between *criterion definition*, *measured value*, and *requirement* that should structure the validator's internal types. Reinforces designing the validator as a rule engine with pluggable criteria.

---

## 1.7 Academic repair pipelines: virtual topology, mesh rescue, learning-based

### Source: Sheffer et al., virtual topology / face clustering for model simplification

**Citation.** Sheffer, A. (2001). Model simplification for meshing using face clustering. *Computer-Aided Design, 33*(13), 925-934. https://doi.org/10.1016/S0010-4485(00)00116-0. See also Sheffer, Blacker, Bercovier, et al. on virtual topology operators for meshing.

**Method.** Virtual topology repairs *meshability* without touching geometry. The algorithm clusters the model's faces into regions using geometric indices (angle and distance criteria over the planar facets approximating each face) chosen to suit meshing/analysis. Resulting clusters drive *virtual* merge and collapse operations: small or troublesome faces and edges are merged or suppressed at the topology layer while the underlying surfaces are untouched, so a mesher sees a clean simplified topology over the original geometry. It works on non-manifold, free-form, and large curved models and produces symmetric partitioning of blends/fillets.

**Limitations.** Solves a different problem from watertighting: it hides small features for meshing, it does not close gaps or fix bad geometry. Virtual entities complicate any operation that must touch real geometry. Clustering thresholds need tuning.

**Kernel relevance.** Virtual topology is the *non-destructive* complement to geometric healing. For Keel, the lesson is to support a topology-overlay layer that can merge/suppress small features for downstream consumers (meshing, analysis) without mutating the healed B-rep. It also gives a principled way to handle slivers that are too small to heal but too small to matter: cluster them away virtually.

### Source: removing small features with real CAD operations

**Citation.** (Companion virtual-topology/defeaturing literature.) Venkataraman, S., et al. *Removing small features with real CAD operations.* In CAD/meshing proceedings (Springer). See also the broader defeaturing literature.

**Method.** Where virtual topology hides features, this line of work *actually removes* small features (tiny holes, blends, slivers) by real local operations (suppress, fill, replace) so the resulting B-rep is genuinely simpler. It pairs feature recognition (find the blend/hole) with local boolean/healing edits.

**Limitations.** Real removal can fail or change the model meaningfully; it requires reliable feature recognition. More invasive than virtual topology.

**Kernel relevance.** Connects healing to Keel's local-operations machinery: defeaturing is delete-face plus heal plus extend-and-reintersect. The small-feature removal that healing needs (collapse sliver, fill micro-hole) is the same toolkit as defeaturing, so build it once.

### Source: Attene, Campen & Kobbelt, polygon mesh repairing survey

**Citation.** Attene, M., Campen, M., & Kobbelt, L. (2013). Polygon mesh repairing: An application perspective. *ACM Computing Surveys, 45*(2), Article 15. https://doi.org/10.1145/2431211.2431214

**Method.** The definitive mesh-repair survey, relevant as the *last-resort rescue path* (convert bad B-rep to mesh, repair the mesh robustly, refit surfaces). It splits repair into two families. **Surface-oriented (local) methods** operate directly on the mesh elements: stitch coincident boundaries, fill holes by triangulation or smooth patching, remove non-manifold edges, resolve self-intersections locally. They preserve the original geometry where it is good but offer no global guarantee of watertightness. **Volumetric (global) methods** resample the model into a volume (signed distance field, octree, or a tetrahedralization aligned to the polygons) and extract a guaranteed-manifold, watertight surface; one cited volumetric approach augments the mesh with a tetrahedral space partition aligned to its polygons to fill arbitrarily complex holes. Volumetric methods guarantee a clean watertight result but discard fine detail and the original tessellation/sharp features. The survey frames the choice as accuracy-preserving-but-no-guarantee versus guaranteed-but-lossy.

**Limitations.** It is about meshes, not exact B-rep; using it for B-rep rescue means a lossy mesh round trip and a surface-refitting step that reintroduces approximation. Volumetric methods lose sharp edges unless feature-preserving variants are used.

**Kernel relevance.** Defines Keel's *fallback* when exact healing fails on a hopeless body: facet it, run volumetric repair to guarantee watertightness, then refit analytic/spline surfaces and re-extract a B-rep. This should be an explicit, clearly-labeled lossy path, never the default. The local-vs-volumetric framing also informs the heal-vs-tolerate decision: local repair = heal, volumetric rebuild = the nuclear option.

### Source: learning-based and inference-based B-rep repair (2020-2025)

**Citation.** (Emerging.) Representative work includes B-rep Boolean result repair by correcting intersection edges via an inference procedure (arXiv:2310.10351, 2023) and B-rep learning encoders such as UV-Net (Jayaraman et al., CVPR 2021) that grid-sample faces in (u,v) and run a graph network over the face-adjacency graph.

**Method.** Two threads. (1) *Inference-based geometric repair*: targets the specific defect of bad intersection edges produced by boolean operations, using a rule/inference procedure to detect and correct mis-computed intersection curves rather than learned weights. (2) *Learning-based representation*: encode the B-rep by sampling each face into a regular (u,v) UV-grid of points/normals and building a graph neural network over the topological face-adjacency graph, enabling defect detection and classification. The recurring difficulty both note: associating a network's output back to specific topology entities is hard because geometric information is lost or altered in the conversion to network-friendly form.

**Limitations.** Immature for production healing. Learning approaches detect/classify better than they repair; mapping predictions back to exact B-rep entities is unsolved. Training data for healing is scarce. Inference-based repair is narrow (boolean intersection edges only).

**Kernel relevance.** Not a near-term dependency for Keel. The UV-grid + face-adjacency-graph encoding is, however, a good *diagnostic* representation: Keel could expose it to let ML classifiers triage defect-prone faces, while keeping the actual repair in deterministic geometric code. Watch this space; do not build on it yet.

---

# PART 2: Surface extension

Surface extension is required wherever healing must rebuild geometry beyond an existing patch: delete-face healing (extend neighbors to re-intersect across the hole), tweak/move-face re-intersection, untrimming (recover the maximal underlying surface), and filling face gaps. The methods below trade off continuity, stability, and how far they can safely reach.

## 2.1 Extension constructions for B-spline / NURBS surfaces

### Source: Shetty & White, curvature-continuous extensions for rational B-spline curves and surfaces

**Citation.** Shetty, S., & White, P. R. (1991). Curvature-continuous extensions for rational B-spline curves and surfaces. *Computer-Aided Design, 23*(7), 484-491. https://doi.org/10.1016/0010-4485(91)90046-Y

**Method.** The seminal, still-most-cited extension construction, and the one most CAD/CAM modelers descend from. It extends a rational B-spline curve (and, by tensor-product application along one parameter direction, a surface) **without changing the shape or parameterization of the original**. The geometric idea is reflection: to extend past the endpoint P(1), it computes a target point by reflecting an interior sample P(1 - t_r) about the curve normal at the endpoint, and constructs the extension's control points so the new segment passes smoothly through these reflected targets, achieving curvature (G2) continuity at the join. For surfaces, the curve construction is applied to the boundary control rows/columns, extending the whole patch in u or v. Because the original control points and knots are preserved and only new control points/knots are appended, the original surface is bit-for-bit unchanged inside its old domain.

**Limitations.** Reflection-based extension is geometrically determined and not adjustable: the user cannot steer the extended shape. As with all polynomial extension, reach is limited; far extensions amplify the endpoint's curvature and can fold or self-intersect. Rational weights complicate the construction and can misbehave. It extends along iso-directions only (you extend the u or v boundary), not in an arbitrary direction.

**Kernel relevance.** The right default for Keel's extension service: shape-and-parameterization preserving, G2 at the join, tensor-product friendly. Implement it as the primary `extend_surface(dir, amount, continuity)` for the healing/local-ops path. Cap the extension amount (Section 2.2) and validate the result for folding.

### Source: extension of B-splines by curve unclamping

**Citation.** Shi, K.-L., Yong, J.-H., Sun, J.-G., Paul, J.-C., & Gu, H.-J. (2002). An extension algorithm for B-splines by curve unclamping. *Computer-Aided Design, 34*(undefined), corrected proof. https://doi.org/10.1016/S0010-4485(01)00108-7 (Note: commonly cited as Shi et al., 2002, CAD.)

**Method.** Extends a B-spline by **unclamping** its end knots. A clamped B-spline has its end knot repeated (degree+1) times, which pins the curve to its end control point. Unclamping reduces that multiplicity and recomputes the affected control points (via the de Boor recurrence run in reverse) so the curve is unchanged but its definition now extends naturally past the former clamped endpoint along the polynomial it already implies. To extend to a chosen target parameter, the knot vector is lengthened and the control points are computed so the curve naturally (maximally smoothly) continues. The construction gives the maximum-continuity natural extension of the existing polynomial segment. It generalizes to surfaces by unclamping along the chosen parameter direction.

**Limitations.** As the search summaries note, the unclamped/natural extension is *exclusive*: it is uniquely determined and cannot be adjusted by the user. Being the natural polynomial continuation, it inherits polynomial extrapolation's instability: the further you extend, the more the trailing polynomial diverges from anything sensible. Behavior degrades quickly for high-degree surfaces.

**Kernel relevance.** Unclamping is the cleanest mathematical formulation of "natural extension" and is cheap (pure knot/control-point algebra, no fitting). Keel should implement unclamping as the engine behind short, exact natural extensions (the common case in delete-face healing where neighbors only need to reach slightly past the hole). Pair it with Shetty-White when controlled G2 reach is needed.

### Source: cubic B-spline extension by minimal strain energy

**Citation.** Mo, G.-L., & Zhao, Y.-N. (2006). A new extension algorithm for cubic B-splines based on minimal strain energy. *Journal of Zhejiang University-SCIENCE A, 7*(12), 2043-2049. https://doi.org/10.1631/jzus.2006.A2043

**Method.** Extends a cubic B-spline to a target point while maintaining GC2 (geometric curvature) continuity, choosing the free parameters of the extension by **minimizing strain (bending) energy** of the new segment. A family of cubic interpolation curves satisfies the GC2 join conditions; the energy-minimization picks the fairest member. The extension is then reparameterized to C2 and knot removal tidies the representation; new control points come from solving lower-triangular linear systems.

**Limitations.** Restricted to cubic B-splines. Requires a target point (it is goal-directed extension, not open-ended natural extension). Energy minimization adds cost over reflection/unclamping. Surface generalization is less direct than the tensor-product methods.

**Kernel relevance.** This is the method to reach for when extension must hit a known target (e.g. extend to meet a specific neighbor's boundary during gap fill) and fairness matters. Lower priority than natural/Shetty-White extension but worth having for high-quality gap closure. The "minimize bending energy among admissible extensions" principle generalizes to a fairing pass on any synthesized fill geometry.

### Source: later B-spline extension algorithms (de Boor recurrence; piecewise polynomial)

**Citation.** Lu, Y., & Wang, G. (2015). A B-spline curve extension algorithm. *Science China Information Sciences, 58*(3), 1-9. https://doi.org/10.1007/s11432-015-5322-x. See also Liu et al. (2020), Extending B-spline by piecewise polynomial, *Computer Animation and Virtual Worlds, 31*, e1942.

**Method.** Lu & Wang extrapolate the existing B-spline using the recurrence property of the de Boor algorithm to produce the natural extension, a refinement on the unclamping formulation with cleaner numerical handling. Liu et al. attach a separate piecewise-polynomial extension segment joined with prescribed continuity, decoupling the extension's flexibility from the original curve's representation.

**Limitations.** The de Boor extrapolation, like unclamping, is the natural (non-adjustable) extension with the same far-reach instability. Piecewise-polynomial attachment adds representation complexity (the result is no longer a single clean B-spline unless re-merged).

**Kernel relevance.** Confirms the de Boor recurrence as the numerically preferred way to compute natural extensions; Keel's unclamping implementation should use it. The piecewise-polynomial approach is a fallback when a longer or more controllable extension is needed than a single natural continuation allows.

## 2.2 Extrapolation stability and practical limits

**Synthesis across the extension literature.** Every method above is, at heart, polynomial (or rational) extrapolation, and extrapolation is conditionally stable at best. Key practical facts for Keel:

- **Degradation is super-linear in reach.** A natural/unclamped extension follows the trailing polynomial segment, which was only ever fit to behave well *inside* the domain. Outside, it diverges; the divergence grows faster for higher polynomial degree. A degree-5 surface extends far less safely than a degree-3 one.
- **Curvature amplification.** Reflection-based (Shetty-White) and natural extensions both propagate the endpoint curvature outward. A patch with high boundary curvature can fold back on itself when extended even modestly, producing a self-intersecting surface that is useless for re-intersection.
- **Rational complications.** For NURBS with non-uniform weights, extension must extend the weight function too. Extrapolated weights can go zero or negative, which makes the rational surface blow up to infinity or flip sign (the surface leaves the convex hull of its control points). Any rational extension must check that extended weights stay strictly positive.
- **Practical length limit.** Kernels therefore cap extension to a small multiple of local feature size (often a fraction of the patch's own dimension), enough to re-intersect a neighbor across a deleted face but no more. When the required reach exceeds the safe cap, the right move is not to extend further but to **replace the extension with an analytic continuation** (if the surface was recognized as, or is close to, an analytic, extend the analytic exactly: a plane, cylinder, etc., extends to infinity with zero error) or to construct dedicated fill geometry.
- **When to extend vs replace.** Extend when the reach is short and the surface is well-behaved (low degree, positive weights, modest curvature). Replace with analytic continuation when the surface is analytic-recognizable. Build new fill geometry (energy-minimizing patch, lofted/blended surface) when the gap is large or the neighbors are too dissimilar to meet via extension.

**Kernel relevance.** Keel's extension service must be *bounded and validated*, never open-ended. Compute a safe reach from local feature size, degree, boundary curvature, and (for rationals) weight positivity; refuse or fall back when the request exceeds it. Always validate the extended patch for folding/self-intersection before handing it to re-intersection. Prefer exact analytic continuation whenever the surface is analytic.

## 2.3 Extension in the healing and local-operations context

### Source: Parasolid PK_SURF / surface extension and delete-face healing

**Citation.** Siemens Digital Industries Software. *Parasolid documentation: surface extension and body healing* (e.g. extend-surface functionality and the delete-face local operation; Parasolid V34.1 release notes on healing surrounding model after face deletion, including for faceted data). Engineering.com, *What's most important in Parasolid 34.1* (2021).

**Method.** In a production kernel, surface extension exists to serve **delete-face healing**: when a face is removed, the kernel extends each surface bordering the wound until those surfaces overlap, re-intersects them to form new edges, and trims a new shell that closes the hole. Parasolid exposes surface extension as a primitive (extend a surface by a parameter/distance, with options for tangential vs natural extension) and uses it inside local operations. The documented requirement is that surfaces must extend *far enough to mutually intersect across the deleted region* but the extension is computed conservatively. Parasolid 34.1 extended this delete-face-then-heal capability even to faceted data. Known failure patterns documented in user material: a surface "getting a major twist" on extension (the fold/curvature-amplification problem), where a tangential-extension option is offered as a mitigation, and cases where neighbors simply cannot be made to intersect (gap too large, surfaces too divergent), where the operation must fail rather than produce garbage.

**Method (ACIS analog).** ACIS similarly provides surface-extension functionality used by its local operations (remove-face, tweak). The pattern is identical: extend, re-intersect, retrim.

**Limitations.** Extension-based healing fails exactly where Section 2.2 predicts: large wounds, high-curvature neighbors, surfaces whose natural extensions diverge before meeting. Tangential extension trades shape fidelity for stability. The operation is only as robust as the underlying surface-surface intersector.

**Kernel relevance.** This is the *raison d'etre* of extension in Keel and pins the requirements: the extension service must (1) extend a bordering surface just past the wound, (2) offer both natural and tangential modes, (3) report failure cleanly when neighbors cannot meet, and (4) feed a robust surface-surface intersector that produces the new edges. Delete-face heal = extend neighbors + re-intersect + retrim, and tweak/move-face is the same machinery. Build extension and intersection as the shared core of local operations and import healing alike.

## 2.4 Untrimming: maximal natural surface recovery

**Synthesis.** Untrimming recovers the maximal underlying (untrimmed) surface from a trimmed face. Two cases. (1) The face's surface already spans more than the trim loop uses: untrimming just discards the trim loop and exposes the full natural surface domain, no extension needed. (2) The desired untrimmed region exceeds the surface's current domain: untrimming requires *extending* the surface (Sections 2.1-2.2) to cover the larger region, then re-trimming to the new (larger or absent) boundary. Untrimming is needed in healing to recover clean base surfaces for re-intersection, in defeaturing (remove a feature, expose the surface it sat on), and in reverse-engineering workflows. The stability limits of Section 2.2 apply directly: you can only untrim-by-extension as far as the surface safely extends.

**Kernel relevance.** Keel should expose `untrim_face` that returns the maximal natural surface, internally calling the extension service when the natural domain must grow. It shares all of extension's safety machinery (reach bounds, fold validation, analytic-continuation preference). Untrimming, delete-face healing, and tweak therefore all sit on the same extension + intersection foundation.

---

# Healing pipeline architecture for Keel

The literature converges on a remarkably consistent architecture. Keel should adopt it with a Rust-idiomatic, report-everything, fail-cleanly design.

## Phase ordering (the healing pipeline)

Synthesized from ACIS HEAL (stitch / simplify / geombuild), OCCT Shape Healing (finest-to-coarsest fix hierarchy), and CADfix (diagnose / repair / validate):

1. **Import and record source tolerance.** Read the body; capture the source system's stated linear and angular tolerances. Never assume Keel's own tolerance. Build the initial topology as a face soup with whatever pcurves arrived.

2. **Diagnose.** Run the full morphologic checklist (Section 1.1): free/non-manifold edges (green/red), vertex/edge/face gaps, degenerate and sliver faces, missing/bad pcurves, self-intersecting trim loops, inconsistent orientations. Emit a structured `HealReport` (criterion, location, severity), using the PDQ/ISO-10303-59 criterion-measurement-requirement structure. Repair is a function of this report.

3. **Stitch (topology reconstruction).** Edge-match under tolerance: merge coincident edge groups into shared edges and coincident vertex groups into shared vertices (ACIS/OCCT stitch). Use Barequet-Sharir's "match what pairs up, then fill the residue" decomposition. Escalate tolerance in controlled steps up to a hard cap (CADfix adaptive widening), recording every widening. Produce a connected shell.

4. **Fix wires and edges (finest-to-coarsest).** Apply OCCT's ordered wire fixes: reorder, drop small edges, force connected, reconcile edge curves (regenerate missing pcurves by robust 3D-to-(u,v) projection; fix reversed/shifted/period pcurves; enforce SameParameter), insert degeneracies at poles, resolve 2D self-intersections, fill parametric gaps. Then face-level (wire orientation, missing seams, tiny-wire removal) and shell-level (coherent orientation).

5. **Simplify (analytic recognition).** Recognize spline patches that are within tolerance of plane/cylinder/cone/sphere/torus and replace them with exact analytics (ACIS geometry simplification). Merge tiny collinear edges and collapse sliver faces. Do this *before* geometry building so re-intersection is exact where possible. Gate every conversion on a verified deviation bound; record into the report so persistent naming can adapt.

6. **Build geometry / tighten gaps (geombuild).** Recompute edges by re-intersecting adjacent surfaces (extending where needed, Part 2). Use a graph-based tangency solver for analytic-analytic junctions (ACIS pattern) and control-point correction for isospline cases. Close residual gaps. This is where the extension service is consumed.

7. **Decide heal-vs-tolerate for residuals** (policy below).

8. **Validate.** Re-run diagnosis. Verify healing preserved shape within tolerance (compare against the original within a bound). Report pass/fail per PDQ criterion. Optionally attach ISO-10303-59 quality declarations on export.

9. **Fallback (rare, lossy, explicit).** If exact healing fails on a hopeless body, offer the mesh-rescue path (Attene): facet, volumetric-repair to guaranteed watertightness, refit surfaces, re-extract B-rep. Never the default; always labeled lossy.

A virtual-topology overlay (Sheffer) can ride on top of the healed body to suppress sub-tolerance slivers for downstream meshing without mutating the B-rep.

## Heal-vs-tolerate policy

The central decision (Parasolid tolerant modeling vs exact healing):

- **Make the B-rep tolerance-aware from day one.** Adopt per-entity geometric tolerances (face/edge/vertex), the Parasolid model. Exact-only kernels are brittle on real imports; a tolerance-aware B-rep is what lets imperfect data survive.
- **Heal first, tolerate second.** Always attempt exact repair (stitch, refit pcurve, re-intersect) within the tolerance cap. Only when a defect cannot be removed within the cap do you record it as a tolerant edge/vertex (tube/sphere model).
- **Prefer re-tightening.** When later geometry permits, replace tolerant entities with exact ones (Parasolid accurate-edges). Track tolerant entities so booleans know they are spreading tolerance, and offer a "re-tighten" pass.
- **Cap and record everything.** Every tolerance escalation and every tolerant entity goes in the report. Callers must be able to see exactly how much the model was perturbed and where precision is soft.
- **Choose the prefer-exact bias for booleanability.** Tolerant entities are contagious in booleans, so bias toward exact healing for any body destined for heavy boolean work; accept more tolerant entities for bodies headed only to meshing/visualization.

## Extension service requirements

Extension is a shared core service for import healing, delete-face healing, tweak/move-face, untrimming, and gap fill. Requirements:

- **API.** `extend_surface(surface, direction{u+/u-/v+/v-}, amount, continuity{G1|G2}, mode{natural|tangential})` returning a new surface or a clean failure.
- **Engines.** Natural extension via knot unclamping / de Boor recurrence (Shi et al.; Lu & Wang) for short exact continuations; reflection-based G2 extension (Shetty & White) for controlled-reach curvature-continuous extension; energy-minimizing extension (Mo & Zhao) for fair extension to a known target.
- **Bounded and validated.** Compute a safe reach from local feature size, surface degree, boundary curvature, and rational weight positivity. Refuse or fall back when the request exceeds it. Always validate the result for folding/self-intersection before use.
- **Prefer analytic continuation.** If the surface is (or is recognized as) analytic, extend the analytic exactly (planes, cylinders, etc. extend to infinity with zero error) instead of extrapolating a spline.
- **Fail cleanly.** When neighbors cannot be made to meet (gap too large, surfaces too divergent), the operation must fail with a clear reason, never produce a folded or self-intersecting result. This is the documented Parasolid/ACIS behavior and the correct one.
- **Co-designed with the intersector.** Extension exists to enable re-intersection; build extension and the surface-surface intersector together as the shared foundation of local operations and healing.

---

# References

1. Attene, M., Campen, M., & Kobbelt, L. (2013). Polygon mesh repairing: An application perspective. *ACM Computing Surveys, 45*(2), Article 15.
2. Barequet, G., & Sharir, M. (1995). Filling gaps in the boundary of a polyhedron. *Computer Aided Geometric Design, 12*(2), 207-229.
3. CAD Interop. *Process automation with CADfix repair* (product/technical documentation). https://www.cadinterop.com/
4. Gonzalez-Lluch, C., Company, P., Contero, M., Camba, J. D., & Plumed, R. (2017). A survey on 3D CAD model quality assurance and testing tools. *Computer-Aided Design, 83*, 64-79. (arXiv:1611.01765.)
5. International TechneGroup Inc. (ITI / Wipro). *CADfix and CADfix DX* documentation. https://www.iti-global.com/interoperability-products/cadfix/
6. ISO. (2022). *ISO 10303-59: Quality of product shape data.* International Organization for Standardization.
7. Jayaraman, P. K., et al. (2021). UV-Net: Learning from boundary representations. *CVPR 2021.*
8. Liu, Y., et al. (2020). Extending B-spline by piecewise polynomial. *Computer Animation and Virtual Worlds, 31*, e1942.
9. Lu, Y., & Wang, G. (2015). A B-spline curve extension algorithm. *Science China Information Sciences, 58*(3), 1-9.
10. Mo, G.-L., & Zhao, Y.-N. (2006). A new extension algorithm for cubic B-splines based on minimal strain energy. *Journal of Zhejiang University-SCIENCE A, 7*(12), 2043-2049.
11. Open CASCADE Technology. (2024). *Shape Healing user guide* and *BRepBuilderAPI_Sewing reference.* Open Cascade SAS.
12. Shetty, S., & White, P. R. (1991). Curvature-continuous extensions for rational B-spline curves and surfaces. *Computer-Aided Design, 23*(7), 484-491.
13. Shi, K.-L., Yong, J.-H., Sun, J.-G., Paul, J.-C., & Gu, H.-J. (2002). An extension algorithm for B-splines by curve unclamping. *Computer-Aided Design, 34.*
14. Sheffer, A. (2001). Model simplification for meshing using face clustering. *Computer-Aided Design, 33*(13), 925-934.
15. Sheffer, A., Blacker, T., Bercovier, M., et al. *Virtual topology operators for meshing* (CAD/meshing literature).
16. Siemens Digital Industries Software. *Parasolid functional description: tolerant modeling, body healing, surface extension, checking* (V34.1/V35 documentation, 2021-2022).
17. Spatial Corp. (Dassault Systemes). *3D ACIS Modeler Healing Component (HEAL)* documentation; *How the 3D ACIS Modeler improves the quality of translated 3D data* (2021).
18. SASIG. *Product Data Quality Guidelines for the Global Automotive Industry*, V2.1 (ISO/PAS 26183:2006); derived from VDA 4955.
19. Theorem Solutions / Elysium. *CADverter, ASFALIS, CADdoctor* interoperability and PDQ healing documentation.
20. Venkataraman, S., et al. *Removing small features with real CAD operations* (defeaturing literature, Springer).
21. Yang, J., Han, S., & Kang, T. W. (2010). PDQ: Representation of data quality for product data and specifically for shape data. *Journal of Computing and Information Science in Engineering, 10*(2), 021003.
22. (Emerging) B-rep Boolean result repair by correcting intersection edges via an inference procedure (arXiv:2310.10351, 2023).
